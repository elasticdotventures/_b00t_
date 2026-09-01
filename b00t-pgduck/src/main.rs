//! Postgres-wire-protocol server backed by DuckDB (b00t#191 pods/postgres
//! replacement). Adapted from pgwire's own sqlite.rs example
//! (github.com/sunng87/pgwire, examples/sqlite.rs) - duckdb-rs mirrors
//! rusqlite's Connection/Rows/ValueRef shape closely enough that this is a
//! direct port, not a rewrite.
//!
//! DUCKDB_PATH controls where the database file lives - point it at a path
//! under an S3-backed FUSE mount (mountpoint-s3/s3fs) for durable storage
//! with no local block volume needed; DuckDB opens it like any other file
//! path, nothing S3-specific in this code. Defaults to in-memory.
//!
//! Known gap, same honesty as the upstream example: only scalar types are
//! mapped (see arrow_type_to_pg_type/encode_row_data) - DuckDB's List/
//! Struct/Array/Map/Union/Enum types aren't handled (relevant for vector
//! columns - see below, they're LIST(FLOAT) under the hood).
//!
//! pgvector-compatible search: DuckDB's `<=>` (cosine distance) and `<#>`
//! (negative inner product) operators were deliberately designed by DuckDB
//! to match pgvector's own operator choices and semantics exactly (see
//! duckdb.org/2024/05/03/vector-similarity-search-vss and the 2024-10-23
//! VSS follow-up post). Since this server does zero query rewriting
//! (NoopQueryParser, raw passthrough to DuckDB's own parser), a client
//! issuing pgvector-style `ORDER BY embedding <=> $1` SQL should work
//! unmodified - no translation layer needed here. `<->` (pgvector's L2/
//! euclidean operator) is NOT one of DuckDB's infix operators; use the
//! `array_distance(a, b)` function instead for L2 distance. HNSW-indexed
//! (as opposed to brute-force) search needs the VSS extension loaded in
//! DuckDB (`INSTALL vss; LOAD vss;`), not yet automated here.

use std::env;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use duckdb::types::ValueRef;
use duckdb::{Connection, Rows, Statement, ToSql};
use futures::{Stream, stream};
use tokio::net::TcpListener;
use tracing::instrument;

use pgwire::api::auth::md5pass::{Md5PasswordAuthStartupHandler, hash_md5_password};
use pgwire::api::auth::{
    AuthSource, DefaultServerParameterProvider, LoginInfo, Password, StartupHandler,
};
use pgwire::api::portal::{Format, Portal};
use pgwire::api::query::{ExtendedQueryHandler, SimpleQueryHandler};
use pgwire::api::results::{
    DataRowEncoder, DescribePortalResponse, DescribeStatementResponse, FieldInfo, QueryResponse,
    Response, Tag,
};
use pgwire::api::stmt::{NoopQueryParser, StoredStatement};
use pgwire::api::{ClientInfo, PgWireServerHandlers, Type};
use pgwire::error::{PgWireError, PgWireResult};
use pgwire::messages::data::DataRow;
use pgwire::tokio::process_socket;

pub struct DuckDbBackend {
    conn: Arc<Mutex<Connection>>,
    query_parser: Arc<NoopQueryParser>,
}

#[derive(Debug)]
struct DummyAuthSource;

// MD5 challenge-response with a fixed password, same posture as the
// upstream example - swap for a real AuthSource (env-var password, or a
// lookup against Key Vault) before this ever leaves localhost/a private
// network. Not done here - out of scope for the prototype.
#[async_trait]
impl AuthSource for DummyAuthSource {
    async fn get_password(&self, login_info: &LoginInfo) -> PgWireResult<Password> {
        println!("login info: {login_info:?}");

        let salt = vec![0, 0, 0, 0];
        let password = env::var("PGDUCK_PASSWORD").unwrap_or_else(|_| "pencil".to_owned());

        let hash_password =
            hash_md5_password(login_info.user().as_ref().unwrap(), &password, salt.as_ref());
        Ok(Password::new(Some(salt), hash_password.as_bytes().to_vec()))
    }
}

#[async_trait]
impl SimpleQueryHandler for DuckDbBackend {
    #[instrument(skip(self, _client), fields(otel.kind = "server", db.system = "duckdb"))]
    async fn do_query<C>(&self, _client: &mut C, query: &str) -> PgWireResult<Vec<Response>>
    where
        C: ClientInfo + Unpin + Send + Sync,
    {
        tracing::info!(query, "simple_query");
        let conn = self.conn.lock().unwrap();
        if query.to_uppercase().starts_with("SELECT") {
            let mut stmt = conn
                .prepare(query)
                .map_err(|e| PgWireError::ApiError(Box::new(e)))?;
            let header = Arc::new(row_desc_from_stmt(&stmt)?);
            stmt.query(duckdb::params![])
                .map(|rows| {
                    let s = encode_row_data(rows, header.clone());
                    vec![Response::Query(QueryResponse::new(header, s))]
                })
                .map_err(|e| PgWireError::ApiError(Box::new(e)))
        } else {
            conn.execute(query, duckdb::params![])
                .map(|affected_rows| {
                    vec![Response::Execution(Tag::new("OK").with_rows(affected_rows))]
                })
                .map_err(|e| PgWireError::ApiError(Box::new(e)))
        }
    }
}

// DuckDB's column_type() returns arrow's DataType, not a decl-type string
// like SQLite - map the common scalars pgwire's wire protocol has a real
// Type for; unmapped kinds (List/Struct/Array/Map/Union/Dictionary/etc)
// fall back to UNKNOWN rather than erroring, same "not all types" honesty
// as the upstream sqlite.rs example this was adapted from.
fn arrow_type_to_pg_type(dt: &duckdb::arrow::datatypes::DataType) -> Type {
    use duckdb::arrow::datatypes::DataType;
    match dt {
        DataType::Boolean => Type::BOOL,
        DataType::Int8 | DataType::UInt8 => Type::CHAR,
        DataType::Int16 | DataType::UInt16 => Type::INT2,
        DataType::Int32 | DataType::UInt32 => Type::INT4,
        DataType::Int64 | DataType::UInt64 => Type::INT8,
        DataType::Float32 => Type::FLOAT4,
        DataType::Float64 => Type::FLOAT8,
        DataType::Utf8 | DataType::LargeUtf8 => Type::TEXT,
        DataType::Binary | DataType::LargeBinary => Type::BYTEA,
        DataType::Date32 | DataType::Date64 => Type::DATE,
        DataType::Timestamp(_, _) => Type::TIMESTAMP,
        _ => Type::UNKNOWN,
    }
}

fn row_desc_from_stmt(stmt: &Statement) -> PgWireResult<Vec<FieldInfo>> {
    let format = Format::UnifiedText;
    (0..stmt.column_count())
        .map(|idx| {
            let name = stmt
                .column_name(idx)
                .map_err(|e| PgWireError::ApiError(Box::new(e)))?
                .to_owned();
            let field_type = arrow_type_to_pg_type(&stmt.column_type(idx));
            Ok(FieldInfo::new(name, None, None, field_type, format.format_for(idx)))
        })
        .collect()
}

fn encode_row_data(
    mut rows: Rows,
    schema: Arc<Vec<FieldInfo>>,
) -> impl Stream<Item = PgWireResult<DataRow>> + use<> {
    let mut results = Vec::new();
    let ncols = schema.len();
    while let Ok(Some(row)) = rows.next() {
        let mut encoder = DataRowEncoder::new(schema.clone());
        for idx in 0..ncols {
            let data = row.get_ref_unwrap::<usize>(idx);
            match data {
                ValueRef::Null => encoder.encode_field(&None::<i8>).unwrap(),
                ValueRef::Boolean(b) => encoder.encode_field(&b).unwrap(),
                ValueRef::TinyInt(i) => encoder.encode_field(&i).unwrap(),
                ValueRef::SmallInt(i) => encoder.encode_field(&i).unwrap(),
                ValueRef::Int(i) => encoder.encode_field(&i).unwrap(),
                ValueRef::BigInt(i) => encoder.encode_field(&i).unwrap(),
                ValueRef::UTinyInt(i) => encoder.encode_field(&(i as i16)).unwrap(),
                ValueRef::USmallInt(i) => encoder.encode_field(&(i as i32)).unwrap(),
                ValueRef::UInt(i) => encoder.encode_field(&(i as i64)).unwrap(),
                ValueRef::UBigInt(i) => encoder.encode_field(&(i as i64)).unwrap(),
                ValueRef::Float(f) => encoder.encode_field(&f).unwrap(),
                ValueRef::Double(f) => encoder.encode_field(&f).unwrap(),
                ValueRef::Text(t) => {
                    encoder.encode_field(&String::from_utf8_lossy(t).as_ref()).unwrap();
                }
                ValueRef::Blob(b) => encoder.encode_field(&b).unwrap(),
                // HugeInt/UHugeInt/Decimal/Timestamp/Date32/Time64/Interval/
                // List/Enum/Struct/Array/Map/Union/Geometry: not mapped in
                // this prototype - encode as text via the value's own
                // Display where practical would be the next increment.
                _ => encoder.encode_field(&None::<i8>).unwrap(),
            }
        }
        results.push(encoder.finish());
    }
    stream::iter(results)
}

fn get_params(portal: &Portal<String>) -> Vec<Box<dyn ToSql>> {
    let mut results = Vec::with_capacity(portal.parameter_len());
    for i in 0..portal.parameter_len() {
        let param_type = portal
            .statement
            .parameter_types
            .get(i)
            .unwrap()
            .as_ref()
            .unwrap_or(&Type::UNKNOWN);
        match param_type {
            &Type::BOOL => {
                results.push(Box::new(portal.parameter::<bool>(i, param_type).unwrap()) as Box<dyn ToSql>);
            }
            &Type::INT2 => {
                results.push(Box::new(portal.parameter::<i16>(i, param_type).unwrap()) as Box<dyn ToSql>);
            }
            &Type::INT4 => {
                results.push(Box::new(portal.parameter::<i32>(i, param_type).unwrap()) as Box<dyn ToSql>);
            }
            &Type::INT8 => {
                results.push(Box::new(portal.parameter::<i64>(i, param_type).unwrap()) as Box<dyn ToSql>);
            }
            &Type::TEXT | &Type::VARCHAR => {
                results.push(Box::new(portal.parameter::<String>(i, param_type).unwrap()) as Box<dyn ToSql>);
            }
            &Type::FLOAT4 => {
                results.push(Box::new(portal.parameter::<f32>(i, param_type).unwrap()) as Box<dyn ToSql>);
            }
            &Type::FLOAT8 => {
                results.push(Box::new(portal.parameter::<f64>(i, param_type).unwrap()) as Box<dyn ToSql>);
            }
            _ => unimplemented!("parameter type not supported"),
        }
    }
    results
}

#[async_trait]
impl ExtendedQueryHandler for DuckDbBackend {
    type Statement = String;
    type QueryParser = NoopQueryParser;

    fn query_parser(&self) -> Arc<Self::QueryParser> {
        self.query_parser.clone()
    }

    #[instrument(skip(self, _client, portal), fields(otel.kind = "server", db.system = "duckdb"))]
    async fn do_query<C>(
        &self,
        _client: &mut C,
        portal: &Portal<Self::Statement>,
        _max_rows: usize,
    ) -> PgWireResult<Response>
    where
        C: ClientInfo + Unpin + Send + Sync,
    {
        let conn = self.conn.lock().unwrap();
        let query = &portal.statement.statement;
        tracing::info!(query = query.as_str(), "extended_query");
        let mut stmt = conn.prepare(query).map_err(|e| PgWireError::ApiError(Box::new(e)))?;
        let params = get_params(portal);
        let params_ref: Vec<&dyn ToSql> = params.iter().map(|f| f.as_ref()).collect();

        if query.to_uppercase().starts_with("SELECT") {
            let header = Arc::new(row_desc_from_stmt(&stmt)?);
            stmt.query(params_ref.as_slice())
                .map(|rows| {
                    let s = encode_row_data(rows, header.clone());
                    Response::Query(QueryResponse::new(header, s))
                })
                .map_err(|e| PgWireError::ApiError(Box::new(e)))
        } else {
            stmt.execute(params_ref.as_slice())
                .map(|affected_rows| Response::Execution(Tag::new("OK").with_rows(affected_rows)))
                .map_err(|e| PgWireError::ApiError(Box::new(e)))
        }
    }

    async fn do_describe_statement<C>(
        &self,
        _client: &mut C,
        stmt: &StoredStatement<Self::Statement>,
    ) -> PgWireResult<DescribeStatementResponse>
    where
        C: ClientInfo + Unpin + Send + Sync,
    {
        let conn = self.conn.lock().unwrap();
        let param_types = stmt
            .parameter_types
            .iter()
            .map(|t| t.clone().unwrap_or(Type::UNKNOWN))
            .collect();
        let prepared = conn
            .prepare(&stmt.statement)
            .map_err(|e| PgWireError::ApiError(Box::new(e)))?;
        row_desc_from_stmt(&prepared).map(|fields| DescribeStatementResponse::new(param_types, fields))
    }

    async fn do_describe_portal<C>(
        &self,
        _client: &mut C,
        portal: &Portal<Self::Statement>,
    ) -> PgWireResult<DescribePortalResponse>
    where
        C: ClientInfo + Unpin + Send + Sync,
    {
        let conn = self.conn.lock().unwrap();
        let stmt = conn
            .prepare(&portal.statement.statement)
            .map_err(|e| PgWireError::ApiError(Box::new(e)))?;
        row_desc_from_stmt(&stmt).map(DescribePortalResponse::new)
    }
}

impl DuckDbBackend {
    fn new() -> DuckDbBackend {
        let conn = match env::var("DUCKDB_PATH") {
            // Point at a path under an S3-backed FUSE mount for durable
            // storage - DuckDB just sees a regular file, no S3 SDK code
            // needed here. Falls back to in-memory if unset.
            Ok(path) => Connection::open(&path)
                .unwrap_or_else(|e| panic!("failed to open DuckDB at {path}: {e}")),
            Err(_) => Connection::open_in_memory().unwrap(),
        };
        DuckDbBackend {
            conn: Arc::new(Mutex::new(conn)),
            query_parser: Arc::new(NoopQueryParser::new()),
        }
    }
}

struct DuckDbBackendFactory {
    handler: Arc<DuckDbBackend>,
}

impl PgWireServerHandlers for DuckDbBackendFactory {
    fn simple_query_handler(&self) -> Arc<impl SimpleQueryHandler> {
        self.handler.clone()
    }

    fn extended_query_handler(&self) -> Arc<impl ExtendedQueryHandler> {
        self.handler.clone()
    }

    fn startup_handler(&self) -> Arc<impl StartupHandler> {
        let mut parameters = DefaultServerParameterProvider::default();
        let version = self.handler.conn.lock().unwrap().version().unwrap_or_else(|_| "unknown".to_owned());
        parameters.server_version = format!("b00t-pgduck/duckdb-{version}");

        Arc::new(Md5PasswordAuthStartupHandler::new(
            Arc::new(DummyAuthSource),
            Arc::new(parameters),
        ))
    }
}

// Initializes an OTLP tracing pipeline if OTEL_EXPORTER_OTLP_ENDPOINT is
// set (standard OTel env var - see
// https://opentelemetry.io/docs/languages/sdk-configuration/otlp-exporter/),
// otherwise falls back to plain stderr logging via tracing-subscriber so
// the server is still usable/observable with no collector running.
fn init_tracing() {
    use opentelemetry::global;
    use opentelemetry::trace::TracerProvider as _;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let fmt_layer = tracing_subscriber::fmt::layer();
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .build()
            .expect("failed to build OTLP exporter");
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(
                opentelemetry_sdk::Resource::builder()
                    .with_service_name("b00t-pgduck")
                    .build(),
            )
            .build();
        let tracer = provider.tracer("b00t-pgduck");
        global::set_tracer_provider(provider);
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(otel_layer)
            .init();
        tracing::info!("otel tracing enabled, exporting to OTEL_EXPORTER_OTLP_ENDPOINT");
    } else {
        tracing_subscriber::registry().with(env_filter).with(fmt_layer).init();
        tracing::info!("OTEL_EXPORTER_OTLP_ENDPOINT unset - stderr logging only, no otel export");
    }
}

#[tokio::main]
pub async fn main() {
    init_tracing();

    let factory = Arc::new(DuckDbBackendFactory {
        handler: Arc::new(DuckDbBackend::new()),
    });

    let server_addr = env::var("PGDUCK_LISTEN").unwrap_or_else(|_| "0.0.0.0:5432".to_owned());
    let listener = TcpListener::bind(&server_addr).await.unwrap();
    tracing::info!(server_addr, "b00t-pgduck listening");
    loop {
        let incoming_socket = listener.accept().await.unwrap();
        let factory_ref = factory.clone();
        tokio::spawn(async move { process_socket(incoming_socket.0, None, factory_ref).await });
    }
}
