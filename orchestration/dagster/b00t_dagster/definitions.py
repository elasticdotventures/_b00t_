"""b00t plane DAGs.

`ci_build_plane` mirrors .github/workflows/ci-build-plane.yml as a Dagster graph
— the first real DAG on the substrate. One build op publishes a nextest archive
to GCS; four run ops execute partitions from it with no compilation; a collect
op joins. Every op is a `dstack apply` (see dstack.py). This is the seam where
Dagster (sequencing / retries / backfills / monitoring) sits above dstack
(elastic spot capacity + teardown).

Run offline:
  B00T_DSTACK_DRY_RUN=1 dagster job execute -m b00t_dagster.definitions -j ci_build_plane
"""

from dagster import (
    Definitions,
    In,
    RetryPolicy,
    graph,
    op,
)

from .dstack import DstackResource, tracer

PARTITIONS = ["1/4", "2/4", "3/4", "4/4"]
_SPOT_RETRY = RetryPolicy(max_retries=2, delay=10)  # ride out a spot eviction


def _archive_key(context) -> str:
    return f"ci-archives/dagster-{context.run_id[:12]}"


@op(retry_policy=_SPOT_RETRY, config_schema={"ci_ref": str})
def build_archive(context, dstack: DstackResource) -> str:
    """1 Spot VM: compile once, publish {nextest,src}.tar.zst to GCS."""
    key = _archive_key(context)
    with tracer().start_as_current_span("ci.build_archive"):
        dstack.apply(
            "dev-env/ci-build.task.yaml",
            name=f"dg-ci-build-{context.run_id[:8]}",
            env={"CI_REF": context.op_config["ci_ref"], "CI_ARCHIVE_KEY": key},
        )
    context.log.info(f"archive published -> {key}")
    return key


def _make_run_partition(partition: str):
    slug = partition.replace("/", "_")

    @op(
        name=f"run_{slug}",
        retry_policy=_SPOT_RETRY,
        config_schema={"ci_ref": str},
        ins={"archive_key": In(str)},
    )
    def _run(context, dstack: DstackResource, archive_key: str):
        """1 compile-free Spot VM: nextest run --archive-file --partition."""
        with tracer().start_as_current_span("ci.run_partition") as span:
            span.set_attribute("nextest.partition", partition)
            dstack.apply(
                "dev-env/ci-test.task.yaml",
                name=f"dg-ci-test-{context.run_id[:8]}-p{slug}",
                env={
                    "CI_ARCHIVE_KEY": archive_key,
                    "CI_REF": context.op_config["ci_ref"],
                    "NEXTEST_PARTITION": partition,
                },
            )
        return f"p{partition} ok"

    return _run


_RUN_OPS = [_make_run_partition(p) for p in PARTITIONS]


@op(ins={"results": In(list)})
def collect(context, results) -> None:
    context.log.info("build plane run complete: " + "; ".join(results))


@graph
def ci_build_plane():
    key = build_archive()
    collect([run(key) for run in _RUN_OPS])


_CI_REF_CFG = {"config": {"ci_ref": "HEAD"}}
ci_build_plane_job = ci_build_plane.to_job(
    name="ci_build_plane",
    resource_defs={"dstack": DstackResource()},
    config={
        "ops": {
            "build_archive": _CI_REF_CFG,
            **{f"run_{p.replace('/', '_')}": _CI_REF_CFG for p in PARTITIONS},
        }
    },
)

defs = Definitions(jobs=[ci_build_plane_job], resources={"dstack": DstackResource()})
