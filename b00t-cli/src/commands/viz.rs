//! Unified visualization command.

use crate::blessing::BlessingGraph;
use crate::commands::task::load_store;
use crate::datum_utils::{DatumGraph, build_datum_graph, graph_neighbors};
use crate::viz::{
    SceneGraph, SceneTheme, blessing_to_mermaid, blessing_to_rhai_dsl,
    blessing_to_scene, datum_graph_to_mermaid, datum_graph_to_rhai_dsl,
    datum_graph_to_scene, scene_to_ascii, scene_to_cytoscape, scene_to_owl2,
    scene_to_svg, scene_to_sysmlv2, tasks_to_mermaid,
    tasks_to_rhai_dsl, tasks_to_scene,
};
use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Clone)]
pub enum VizCommands {
    #[clap(about = "Visualize a blessing graph TOML file")]
    Blessing {
        #[clap(long, short, help = "Path to a blessing graph TOML file")]
        input: PathBuf,
        #[clap(
            long,
            value_enum,
            default_value = "mermaid",
            help = "Output format; plantuml is deprecated source-only output, no Java renderer or queue"
        )]
        format: VizFormat,
        #[clap(long, short, help = "Write output to file instead of stdout")]
        output: Option<PathBuf>,
    },
    #[clap(about = "Visualize native b00t task dependencies")]
    Task {
        #[clap(
            long,
            value_enum,
            default_value = "mermaid",
            help = "Output format; plantuml is deprecated source-only output, no Java renderer or queue"
        )]
        format: VizFormat,
        #[clap(long, short, help = "Write output to file instead of stdout")]
        output: Option<PathBuf>,
    },
    #[clap(about = "Visualize datum entanglement/dependency graph")]
    Entangle {
        #[clap(long, help = "Seed datum key/name; omitted renders the whole graph")]
        datum: Option<String>,
        #[clap(long, default_value_t = 2, help = "Neighbor depth when --datum is set")]
        depth: usize,
        #[clap(long, default_value = "both", help = "Neighbor direction: out|in|both")]
        direction: String,
        #[clap(
            long,
            value_enum,
            default_value = "mermaid",
            help = "Output format; plantuml is deprecated source-only output, no Java renderer or queue"
        )]
        format: VizFormat,
        #[clap(long, short, help = "Write output to file instead of stdout")]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum VizFormat {
    Isometric,
    Mermaid,
    Rhai,
    Json,
    Ascii,
    Svg,
    Cytoscape,
    SysMLv2,
    Owl2,
}

pub fn handle_viz_command(path: &str, command: &VizCommands) -> Result<()> {
    match command {
        VizCommands::Blessing {
            input,
            format,
            output,
        } => {
            let content = fs::read_to_string(input)
                .with_context(|| format!("read blessing graph {}", input.display()))?;
            let graph = BlessingGraph::from_toml(&content)
                .map_err(|err| anyhow::anyhow!("parse blessing graph: {err}"))?;
            let rendered = render(
                *format,
                blessing_to_mermaid(&graph),
                blessing_to_rhai_dsl(&graph),
                blessing_to_scene(&graph),
            )?;
            emit(rendered, output)
        }
        VizCommands::Task { format, output } => {
            let store = load_store()?;
            let rendered = render(
                *format,
                tasks_to_mermaid(&store.tasks),
                tasks_to_rhai_dsl(&store.tasks),
                tasks_to_scene(&store.tasks),
            )?;
            emit(rendered, output)
        }
        VizCommands::Entangle {
            datum,
            depth,
            direction,
            format,
            output,
        } => {
            let graph = match datum {
                Some(seed) => scoped_datum_graph(path, seed, *depth, direction)?,
                None => build_datum_graph(path, Some(10))?,
            };
            let rendered = render(
                *format,
                datum_graph_to_mermaid(&graph),
                datum_graph_to_rhai_dsl(&graph),
                datum_graph_to_scene(&graph),
            )?;
            emit(rendered, output)
        }
    }
}

fn render(
    format: VizFormat,
    mermaid: String,
    rhai: String,
    scene: SceneGraph,
) -> Result<String> {
    match format {
        VizFormat::Isometric | VizFormat::Svg => Ok(scene_to_svg(&scene, &SceneTheme::default())),
        VizFormat::Mermaid => Ok(format!("```mermaid\n{}```\n", mermaid)),
        VizFormat::Rhai => Ok(format!("```rhai\n{}```\n", rhai)),
        VizFormat::Json => serde_json::to_string_pretty(&scene).context("serialize scene graph"),
        VizFormat::Ascii => Ok(scene_to_ascii(&scene)),
        VizFormat::Cytoscape => Ok(scene_to_cytoscape(&scene)),
        VizFormat::SysMLv2 => Ok(scene_to_sysmlv2(&scene)),
        VizFormat::Owl2 => Ok(scene_to_owl2(&scene)),
    }
}

fn emit(rendered: String, output: &Option<PathBuf>) -> Result<()> {
    if let Some(output) = output {
        fs::write(output, rendered).with_context(|| format!("write {}", output.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn scoped_datum_graph(path: &str, seed: &str, depth: usize, direction: &str) -> Result<DatumGraph> {
    let graph = build_datum_graph(path, Some(10))?;
    let edges = graph_neighbors(&graph, seed, depth, direction);
    let mut keys: HashSet<String> = HashSet::from([seed.to_string()]);
    for edge in &edges {
        keys.insert(edge.from.clone());
        keys.insert(edge.to.clone());
    }
    let nodes = graph
        .nodes
        .into_iter()
        .filter(|node| keys.contains(&node.key) || node.name == seed)
        .collect();

    Ok(DatumGraph { nodes, edges })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn blessing_viz_command_outputs_mermaid() {
        let tmp = tempdir().unwrap();
        let input = tmp.path().join("blessing.toml");
        fs::write(
            &input,
            r#"
[b00t]
name = "blessing-test"
type = "config"

[[b00t.nodes]]
id = "executive"
type = "role"
cost_tokens = 10
role_access = ["executive"]

[[b00t.nodes]]
id = "operator"
type = "role"
cost_tokens = 5
role_access = ["operator"]

[[b00t.edges]]
from = "executive"
to = "operator"
relationship = "delegates"
"#,
        )
        .unwrap();
        let output = tmp.path().join("out.md");
        let command = VizCommands::Blessing {
            input,
            format: VizFormat::Mermaid,
            output: Some(output.clone()),
        };
        handle_viz_command("_b00t_", &command).unwrap();
        let rendered = fs::read_to_string(output).unwrap();
        assert!(rendered.contains("```mermaid"));
        assert!(rendered.contains("delegates"));
    }

    #[test]
    fn render_supports_all_text_formats() {
        let mut scene = SceneGraph::new(1.0);
        scene.add_node(crate::viz::SceneNode {
            id: "a".into(),
            label: "a".into(),
            position: crate::viz::Point3D::new(0.0, 0.0, 0.0),
            role: crate::viz::SemanticRole::Step,
            arm_index: None,
            is_default: true,
        });

        assert!(
            render(
                VizFormat::Mermaid,
                "graph LR\n".into(),
                "".into(),
                scene.clone()
            )
            .unwrap()
            .contains("```mermaid")
        );
        assert!(
            render(
                VizFormat::Rhai,
                "".into(),
                "task(\"1\", {})\n".into(),
                scene.clone()
            )
            .unwrap()
            .contains("```rhai")
        );
        assert!(
            render(VizFormat::Ascii, "".into(), "".into(), scene)
                .unwrap()
                .contains("nodes:")
        );
    }
}
