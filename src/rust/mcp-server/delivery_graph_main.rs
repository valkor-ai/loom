use std::{fs, path::PathBuf};

use delivery_core::ProjectToolInput;
use mcp_server::{delivery_graph, delivery_graph_svg};

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        anyhow::bail!("Usage: loom-delivery-graph <project-root> <output.svg> [--view delivery|architecture] [--phase <phase-id>]");
    }
    let mut architecture = false;
    let mut phase = None;
    let mut options = args[2..].chunks_exact(2);
    for option in &mut options {
        match option[0].as_str() {
            "--view" => match option[1].as_str() {
                "architecture" => architecture = true,
                "delivery" => architecture = false,
                _ => anyhow::bail!("View must be delivery or architecture"),
            },
            "--phase" => phase = Some(option[1].as_str()),
            _ => anyhow::bail!("Unknown option: {}", option[0]),
        }
    }
    anyhow::ensure!(options.remainder().is_empty(), "Each option needs a value");
    anyhow::ensure!(
        architecture || phase.is_none(),
        "--phase is supported with --view architecture"
    );
    let output = PathBuf::from(&args[1]);
    anyhow::ensure!(
        output.extension().is_some_and(|ext| ext == "svg"),
        "Output must be an .svg file"
    );
    let input = ProjectToolInput {
        project_root: args[0].clone(),
    };
    let graph = if architecture {
        delivery_graph::read_architecture(input, phase)?
    } else {
        delivery_graph::read(input)?
    };
    let svg = if architecture {
        delivery_graph_svg::render_architecture(&graph)
    } else {
        delivery_graph_svg::render(&graph)
    };
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, svg)?;
    println!(
        "{} nodes, {} relationships: {}",
        graph.nodes.len(),
        graph.edges.len(),
        output.display()
    );
    Ok(())
}
