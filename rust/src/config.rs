use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub struct Config {
    pub extract: HashMap<String, Vec<String>>,
}

pub fn load_config() -> Result<Config, Box<dyn Error>> {
    let home = PathBuf::from(env::var_os("HOME").expect("HOME is not set"));
    let path = home.join(".tracker").join("config.json");

    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => String::from("{}"),
        Err(err) => return Err(format!("can't read {}: {err}", path.display()).into()),
    };
    match parse(&text, &home) {
        Ok(config) => Ok(config),
        Err(message) => Err(format!("{}: {message}", path.display()).into()),
    }
}

fn parse(text: &str, home: &Path) -> Result<Config, String> {
    let root: Value = match serde_json::from_str(text) {
        Ok(root) => root,
        Err(err) => return Err(err.to_string()),
    };
    if !root.is_object() {
        return Err(String::from(
            "expected an object, like {\"extract\": {...}}",
        ));
    }

    let mut config = Config {
        extract: HashMap::new(),
    };
    match root.get("extract") {
        None => {}
        Some(Value::Object(table)) => {
            for (extension, value) in table {
                let name = format!("extract.{extension}");
                let command = parse_command(value, home, &name)?;
                config.extract.insert(extension.to_lowercase(), command);
            }
        }
        Some(_) => {
            return Err(String::from(
                "extract must be an object of extension: command",
            ));
        }
    }
    Ok(config)
}

fn parse_command(value: &Value, home: &Path, name: &str) -> Result<Vec<String>, String> {
    let usage =
        format!("{name} must be a list of strings, like [\"pdftotext\", \"{{path}}\", \"-\"]");
    let items = match value {
        Value::Array(items) if !items.is_empty() => items,
        _ => return Err(usage),
    };

    let mut command = Vec::new();
    for item in items {
        match item {
            Value::String(arg) => command.push(expand_home(arg, home)),
            _ => return Err(usage),
        }
    }
    Ok(command)
}

fn expand_home(arg: &str, home: &Path) -> String {
    match arg.strip_prefix("~/") {
        Some(rest) => home.join(rest).to_string_lossy().into_owned(),
        None => arg.to_string(),
    }
}
