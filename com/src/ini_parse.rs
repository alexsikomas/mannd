use std::collections::HashMap;

pub struct IniConfig<'a> {
    pub sections: HashMap<&'a str, HashMap<&'a str, &'a str>>,
}

impl<'a> IniConfig<'a> {
    pub fn new() -> Self {
        let sections: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
        Self { sections }
    }

    pub fn parse_file(&mut self, lines: core::str::Lines<'a>) {
        let mut current_section: &str = "";
        for line in lines {
            let line = line.trim();
            // skip whitespace, incorrect indentation and empty lines
            if line.starts_with(['#']) || line.is_empty() {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                current_section = &line[1..line.len() - 1];
                self.sections.entry(current_section).or_default();
            } else if let Some((k, v)) = line.split_once('=') {
                self.sections
                    .entry(current_section)
                    .or_default()
                    .insert(k.trim(), v.trim());
            }
        }
    }
}
