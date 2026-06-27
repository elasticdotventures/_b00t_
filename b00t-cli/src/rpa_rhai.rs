use anyhow::Result;
use rhai::{Array, Dynamic, Engine, Map};
use std::sync::mpsc;
use tokio::sync::mpsc as tmpsc;

pub type CmdTx = tmpsc::UnboundedSender<Cmd>;
pub type CmdRx = tmpsc::UnboundedReceiver<Cmd>;

/// Reply channel for synchronous results from async executor.
type Reply<T> = mpsc::Sender<T>;

pub enum Cmd {
    Nav(String, bool, Reply<bool>),
    Click(String, Reply<bool>),
    Type(String, String, Reply<bool>),
    Eval(String, Reply<String>),
    Screen(String, Reply<bool>),
    Wait(String, Reply<bool>),
    Desc(Reply<String>),
}

pub async fn run_commands(session: &crate::rpa_cdp::RpaSession, rx: &mut CmdRx) {
    let mut current_page: Option<chromiumoxide::Page> = None;

    while let Some(cmd) = rx.recv().await {
        match cmd {
            Cmd::Nav(url, enrich, tx) => {
                match tokio::time::timeout(std::time::Duration::from_secs(20), session.open_page(&url, enrich)).await {
                    Ok(Ok(page)) => { current_page = Some(page); let _ = tx.send(true); }
                    Ok(Err(e)) => { eprintln!("  ⚠️ {}", e); let _ = tx.send(false); }
                    Err(_) => { eprintln!("  ⚠️ Timeout navigating to {}", url); let _ = tx.send(false); }
                }
            }
            Cmd::Click(sel, tx) => {
                let r = if let Some(ref page) = current_page {
                    session.click(page, &sel).await.is_ok()
                } else { false };
                let _ = tx.send(r);
            }
            Cmd::Type(sel, txt, tx) => {
                let r = if let Some(ref page) = current_page {
                    session.type_text(page, &sel, &txt).await.is_ok()
                } else { false };
                let _ = tx.send(r);
            }
            Cmd::Eval(js, tx) => {
                let r = if let Some(ref page) = current_page {
                    session.evaluate(page, &js).await.unwrap_or_default()
                } else { String::new() };
                let _ = tx.send(r);
            }
            Cmd::Screen(path, tx) => {
                let r = if let Some(ref page) = current_page {
                    if let Ok(png) = session.screenshot(page).await {
                        std::fs::write(&path, &png).is_ok()
                    } else { false }
                } else { false };
                let _ = tx.send(r);
            }
            Cmd::Wait(sel, tx) => {
                let r = if let Some(ref page) = current_page {
                    let mut found = false;
                    for _ in 0..20 {
                        let js = format!("!!document.querySelector('{}')", sel);
                        if let Ok(r) = session.evaluate(page, &js).await {
                            if r == "true" { found = true; break; }
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                    found
                } else { false };
                let _ = tx.send(r);
            }
            Cmd::Desc(tx) => {
                let r = if let Some(ref page) = current_page {
                    let t = session.evaluate(page, "document.title").await.unwrap_or_default();
                    let l = session.evaluate(page, "document.querySelectorAll('a').length").await.unwrap_or_default();
                    format!("📄 {}\n   Links: {}", t, l)
                } else { "No page open".into() };
                let _ = tx.send(r);
            }
        }
    }
}

pub fn create_rpa_engine(cmd_tx: CmdTx) -> Engine {
    let mut engine = Engine::new();

    use Reply;
    let mk_reply = || -> (Reply<bool>, mpsc::Receiver<bool>) { mpsc::channel() };
    let mk_str_reply = || -> (Reply<String>, mpsc::Receiver<String>) { mpsc::channel() };

    let t = cmd_tx.clone();
    engine.register_fn("navigate", move |url: &str| -> bool {
        let (tx, rx) = mk_reply();
        t.send(Cmd::Nav(url.into(), false, tx)).ok();
        rx.recv().unwrap_or(false)
    });

    let t = cmd_tx.clone();
    engine.register_fn("navigate_enrich", move |url: &str| -> bool {
        let (tx, rx) = mk_reply();
        t.send(Cmd::Nav(url.into(), true, tx)).ok();
        rx.recv().unwrap_or(false)
    });

    let t = cmd_tx.clone();
    engine.register_fn("click", move |sel: &str| -> bool {
        let (tx, rx) = mk_reply();
        t.send(Cmd::Click(sel.into(), tx)).ok();
        rx.recv().unwrap_or(false)
    });

    let t = cmd_tx.clone();
    engine.register_fn("type_text", move |sel: &str, txt: &str| -> bool {
        let (tx, rx) = mk_reply();
        t.send(Cmd::Type(sel.into(), txt.into(), tx)).ok();
        rx.recv().unwrap_or(false)
    });

    let t = cmd_tx.clone();
    engine.register_fn("evaluate", move |js: &str| -> String {
        let (tx, rx) = mk_str_reply();
        t.send(Cmd::Eval(js.into(), tx)).ok();
        rx.recv().unwrap_or_default()
    });

    let t = cmd_tx.clone();
    engine.register_fn("screenshot", move |path: &str| -> bool {
        let (tx, rx) = mk_reply();
        t.send(Cmd::Screen(path.into(), tx)).ok();
        rx.recv().unwrap_or(false)
    });

    let t = cmd_tx.clone();
    engine.register_fn("wait_for", move |sel: &str| -> bool {
        let (tx, rx) = mk_reply();
        t.send(Cmd::Wait(sel.into(), tx)).ok();
        rx.recv().unwrap_or(false)
    });

    let t = cmd_tx.clone();
    engine.register_fn("describe_page", move || -> String {
        let (tx, rx) = mk_str_reply();
        t.send(Cmd::Desc(tx)).ok();
        rx.recv().unwrap_or_default()
    });

    engine
}

pub fn load_page_model(path: &str) -> Result<Map> {
    let content = std::fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(&content)?;
    Ok(toml_to_map(&value))
}

fn toml_to_map(v: &toml::Value) -> Map {
    let mut m = Map::new();
    if let toml::Value::Table(t) = v {
        for (k, v) in t { m.insert(k.into(), toml_to_dyn(v)); }
    }
    m
}

fn toml_to_dyn(v: &toml::Value) -> Dynamic {
    match v {
        toml::Value::String(s) => s.as_str().into(),
        toml::Value::Integer(i) => (*i).into(),
        toml::Value::Float(f) => (*f).into(),
        toml::Value::Boolean(b) => (*b).into(),
        toml::Value::Array(a) => a.iter().map(toml_to_dyn).collect::<Array>().into(),
        toml::Value::Table(_) => Dynamic::from(toml_to_map(v)),
        _ => Dynamic::UNIT,
    }
}
