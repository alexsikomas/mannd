use std::{
    collections::BTreeMap,
    fs::read_to_string,
    io::{BufWriter, Write},
    path::PathBuf,
};

use crate::{error::ManndError, utils::NamedTempFile};

pub struct IniConfig {
    pub file_path: PathBuf,
    // use btreemap to keep ordering
    pub sections: BTreeMap<String, BTreeMap<String, String>>,
}

impl IniConfig {
    pub fn new(file_path: PathBuf) -> Result<Self, ManndError> {
        let sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let mut conf = Self {
            file_path,
            sections,
        };
        Self::parse_file(&mut conf)?;
        Ok(conf)
    }

    fn parse_file(&mut self) -> Result<(), ManndError> {
        let file = read_to_string(self.file_path.clone())
            .map_err(|_| ManndError::FileNotFound("File not found".to_string()))?;
        let lines = file.lines();

        let mut current_section: String = String::new();
        for line in lines {
            let line = line.trim().to_string();
            // skip whitespace, incorrect indentation and empty lines
            if line.starts_with(['#']) || line.is_empty() {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                self.sections
                    .entry(current_section.to_string())
                    .or_default();
            } else if let Some((k, v)) = line.split_once('=') {
                self.sections
                    .entry(current_section.to_string())
                    .or_default()
                    .insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        Ok(())
    }

    pub fn add_to_section<T: Into<String>>(
        &mut self,
        key: T,
        value: (T, T),
    ) -> Result<(), ManndError> {
        let key_ref: &String = &key.into();
        let section = self.sections.get_mut(key_ref).ok_or_else(|| {
            ManndError::ConfigSectionNotFound(format!("{} not found...", key_ref))
        })?;

        section.insert(value.0.into(), value.1.into());

        Ok(())
    }

    pub fn write_file(&self) -> Result<(), ManndError> {
        let tmp_file = NamedTempFile::new()?;
        let mut writer = BufWriter::new(&tmp_file.file);

        for (section, keys) in &self.sections {
            writeln!(writer, "[{}]", section)?;

            for (key, value) in keys {
                writeln!(writer, "{}={}", key, value)?;
            }
            writeln!(writer, "")?;
        }

        writer.flush()?;

        Ok(())
    }
}
