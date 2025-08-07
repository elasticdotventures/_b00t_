use anyhow::Result;
use b00t_c0re_lib::learn::{get_learn_topics, get_learn_lesson};
use b00t_c0re_lib::TemplateRenderer;

pub fn handle_learn(path: &str, topic: Option<&str>) -> Result<()> {
    match topic {
        None => {
            let topics = get_learn_topics(path)?;
            let renderer = TemplateRenderer::with_defaults()?;
            let topics_json = serde_json::to_string_pretty(&topics)?;
            let rendered = renderer.render(&topics_json)?;
            println!("{}", rendered);
        }
        Some(topic_name) => {
            let lesson = get_learn_lesson(path, topic_name)?;
            let renderer = TemplateRenderer::with_defaults()?;
            let rendered = renderer.render(&lesson)?;
            println!("{}", rendered);
        }
    }
    Ok(())
}
