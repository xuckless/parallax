//! Generated project files: `{{key}}` templates, escaping for the files values land in, and writing them out.

use std::path::Path;

use crate::error::CliError;

/// `template` with every `{{key}}` replaced by its value.
pub fn render(template: &str, vars: &[(&str, String)]) -> String {
    vars.iter()
        .fold(template.to_string(), |text, (key, value)| {
            text.replace(&format!("{{{{{key}}}}}"), value)
        })
}

/// Text for inside an XML (or HTML) element or attribute.
pub fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Writes `bytes` to `path`, making its directory first.
pub fn write(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let io = |source| CliError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    std::fs::write(path, bytes).map_err(io)
}

/// Removes `dir` if it exists, so a generated tree holds no file from an earlier run.
pub fn clear(dir: &Path) -> Result<(), CliError> {
    match dir.exists() {
        true => std::fs::remove_dir_all(dir).map_err(|source| CliError::Io {
            path: dir.to_path_buf(),
            source,
        }),
        false => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_and_escape_table() {
        // (row name, template, vars, expected)
        type Case<'a> = (&'a str, &'a str, Vec<(&'a str, String)>, &'a str);
        let cases: [Case; 3] = [
            (
                "fills every key",
                "{{a}}-{{b}}-{{a}}",
                vec![("a", "1".into()), ("b", "2".into())],
                "1-2-1",
            ),
            (
                "unknown key stays",
                "{{c}}",
                vec![("a", "1".into())],
                "{{c}}",
            ),
            (
                "escaped value",
                "<s>{{n}}</s>",
                vec![("n", xml_escape("Tom & 'J' <x>"))],
                "<s>Tom &amp; &apos;J&apos; &lt;x&gt;</s>",
            ),
        ];
        for (name, template, vars, want) in cases {
            assert_eq!(render(template, &vars), want, "{name}");
        }
    }
}
