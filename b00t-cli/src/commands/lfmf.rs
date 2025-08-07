use anyhow::Result;
use b00t_c0re_lib::learn::record_lesson;

pub fn handle_lfmf(path: &str, tool: &str, lesson: &str) -> Result<()> {
    record_lesson(path, tool, lesson)?;
    println!("Lesson recorded for {}", tool);
    Ok(())
}
