//! b00t-cad — geometry as code via OpenCASCADE (cadrum)
//!
//! Run: cargo run -p b00t-cad -- [part_name]
//! Parts: flange, gear, bolt, heart, spring

use cadrum::{DVec3, Solid};
use std::io::BufWriter;
use std::time::Instant;

fn flange() -> Result<Solid, Box<dyn std::error::Error>> {
    let body = Solid::cylinder(15.0, DVec3::Z * 30.0).color("#4a90d9");
    let mut part = body;
    for i in 0..6 {
        let angle = (i as f64) * std::f64::consts::TAU / 6.0;
        let hole = Solid::cylinder(3.0, DVec3::Z * 35.0).translate(DVec3::new(
            10.0 * angle.cos(),
            10.0 * angle.sin(),
            -2.5,
        ));
        part = (&part - &hole).build()?;
    }
    let bore = Solid::cylinder(5.0, DVec3::Z * 35.0).translate(DVec3::new(0.0, 0.0, -2.5));
    part = (&part - &bore).build()?;
    let base = Solid::cube(DVec3::ZERO, DVec3::new(40.0, 40.0, 5.0))
        .translate(-DVec3::new(20.0, 20.0, 0.0))
        .color("#e8a87c");
    Ok((&part + &base).build()?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let solid = flange()?;
    let mesh = Solid::mesh([&solid], Default::default())?;

    let name = "flange";
    Solid::write_step(
        [&solid],
        &mut BufWriter::new(std::fs::File::create(format!("{name}.step"))?),
    )?;
    mesh.write_stl(&mut BufWriter::new(std::fs::File::create(format!(
        "{name}.stl"
    ))?))?;
    mesh.write_gltf_binary(&mut BufWriter::new(std::fs::File::create(format!(
        "{name}.glb"
    ))?))?;

    println!(
        "{name} — {:.2?} | {} tris | .step .stl .glb",
        start.elapsed(),
        mesh.vertices.len()
    );
    Ok(())
}
