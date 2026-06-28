use axum::{Json, Router, routing::get};
use b00t_cli::DatumType;
use b00t_reflect_types::HolonNode;

pub fn type_graph_router() -> Router {
    Router::new().route("/v1/b00t/type-graph", get(type_graph_handler))
}

async fn type_graph_handler() -> Json<Vec<HolonNode>> {
    Json(DatumType::datum_nodes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn type_graph_returns_200_with_all_variants() {
        let app = type_graph_router();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/b00t/type-graph")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let nodes: Vec<HolonNode> = serde_json::from_slice(&body).unwrap();

        // Every DatumType variant must appear exactly once
        assert_eq!(nodes.len(), DatumType::all_variants().len());

        // Every node must have non-empty id and label
        for node in &nodes {
            assert!(!node.id.is_empty(), "node id must not be empty");
            assert!(!node.label.is_empty(), "node label must not be empty");
        }

        // Spot-check: Skill variant present (id = "datum_type::{type_prefix}")
        assert!(
            nodes.iter().any(|n| n.id == "datum_type::skill"),
            "Skill variant missing from type graph"
        );
    }

    #[tokio::test]
    async fn type_graph_is_idempotent() {
        let app = type_graph_router();

        let r1 = app
            .clone()
            .oneshot(Request::builder().uri("/v1/b00t/type-graph").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let r2 = app
            .oneshot(Request::builder().uri("/v1/b00t/type-graph").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let b1 = axum::body::to_bytes(r1.into_body(), usize::MAX).await.unwrap();
        let b2 = axum::body::to_bytes(r2.into_body(), usize::MAX).await.unwrap();
        assert_eq!(b1, b2, "type-graph must be deterministic across calls");
    }
}
