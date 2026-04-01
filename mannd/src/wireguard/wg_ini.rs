//! Wireguard INI parser
//!
//! Custom INI parser as we only need a minimal amount of features.

use std::{
    collections::HashMap,
    fs::read_to_string,
    io::{self, BufWriter, Write},
    path::Path,
};

use crate::error::ManndError;

pub struct WgConfig {
    pub sections: HashMap<String, HashMap<String, String>>,
}

impl WgConfig {
    pub fn parse(path: &Path) -> Result<Self, ManndError> {
        let ini_str = read_to_string(path)
            .map_err(|_| ManndError::FileNotFound(path.display().to_string()))?;
        Ok(Self::parse_str(&ini_str))
    }

    pub fn parse_str(conf: &str) -> Self {
        let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut current = String::new();

        for line in conf.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                current = line[1..line.len() - 1].to_string();
                sections.entry(current.clone()).or_default();
            } else if let Some((k, v)) = line.split_once('=') {
                sections
                    .entry(current.clone())
                    .or_default()
                    .insert(k.trim().to_string(), v.trim().to_string());
            }
        }

        Self { sections }
    }

    pub fn get(&self, section: &str, field: &str) -> Result<&str, ManndError> {
        self.sections
            .get(section)
            .ok_or_else(|| ManndError::SectionNotFound(section.into()))?
            .get(field)
            .map(String::as_str)
            .ok_or_else(|| ManndError::PropertyNotFound(field.into()))
    }

    pub fn get_partial(&self, filter: &HashMap<String, Vec<String>>) -> Result<Self, ManndError> {
        let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
        for (section, fields) in filter {
            let src_section = self
                .sections
                .get(section)
                .ok_or_else(|| ManndError::SectionNotFound(section.clone()))?;
            let mut kept: HashMap<String, String> = HashMap::new();
            for field in fields {
                let val = src_section
                    .get(field)
                    .ok_or_else(|| ManndError::PropertyNotFound(field.clone()))?;
                kept.insert(field.clone(), val.clone());
            }
            sections.insert(section.clone(), kept);
        }
        Ok(Self { sections })
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        for (section, fields) in &self.sections {
            writeln!(writer, "[{section}]")?;
            for (key, value) in fields {
                writeln!(writer, "{key} = {value}")?;
            }
            writeln!(writer)?;
        }
        writer.flush()
    }

    pub fn write_file(&self, path: &Path) -> Result<(), ManndError> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.write_to(&mut writer)?;
        Ok(())
    }
}
