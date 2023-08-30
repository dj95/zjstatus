use std::collections::BTreeMap;

#[derive(Default)]
pub struct ModuleConfig {
    pub enabled_modules: Vec<String>,
    pub formatted_parts: Vec<FormattedPart>,
}

pub struct FormattedPart {
    pub order: u8,
    pub fg: u8,
    pub bg: u8,
    pub bold: bool,
    pub italic: bool,
    pub content: String,
}

impl FormattedPart {
    pub fn from_format_string(format: String) -> Self {
        let mut result = FormattedPart::default();

        let format_content_split = format.split("]");

        if format_content_split.clone().count() == 1 {
            result.content = format;

            return result;
        }

        let format_content_split = format_content_split.collect::<Vec<&str>>();
        result.content = format_content_split[1].to_string();

        let parts = format_content_split[0].split(",");
        for part in parts {
            if part.starts_with("fg=") {
                // TODO: better error handling
                result.fg = part
                    .strip_prefix("fg=")
                    .unwrap()
                    .to_string()
                    .parse::<u8>()
                    .unwrap();
            }

            if part.starts_with("bg=") {
                // TODO: better error handling
                result.bg = part
                    .strip_prefix("bg=")
                    .unwrap()
                    .to_string()
                    .parse::<u8>()
                    .unwrap();
            }

            result.bold = part.eq("bold");
            result.italic = part.eq("italic");
        }

        result
    }
}

impl Default for FormattedPart {
    fn default() -> Self {
        Self {
            order: 0,
            fg: 255,
            bg: 0,
            bold: false,
            italic: false,
            content: "".to_string(),
        }
    }
}

pub fn parse_format(config: BTreeMap<String, String>) -> ModuleConfig {
    let format = config.get("format");
    let mut formatted_parts = Vec::new();

    if format.is_none() {
        return ModuleConfig {
            enabled_modules: Vec::new(),
            formatted_parts,
        };
    }

    let format = format.unwrap();

    let mut counter: u8 = 0;
    let color_parts = format.split("#[");
    for color_part in color_parts {
        let mut part = FormattedPart::from_format_string(color_part.to_string());
        part.order = counter.clone();

        formatted_parts.push(part);

        counter += 1;
    }

    ModuleConfig {
        enabled_modules: Vec::new(),
        formatted_parts,
    }
}
