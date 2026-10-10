use super::load_types::{
    FileControlState, LoadDiagnostic, LoadFailure, LoadInput, LoadLabel, LoadStatus,
};

pub(super) struct File<'a> {
    pub cargo: Option<super::load_cargo_translation::Table>,
    pub language_maps: std::collections::BTreeMap<u32, super::load_language::LanguageMap>,
    pub features: u32,
    pub globals: super::load_context::FileGlobals,
    pub name: &'a str,
    pub grfid: u32,
    pub version: u8,
    pub parameters: Vec<u32>,
    pub labels: Vec<LoadLabel>,
}
pub(super) struct Config<'a> {
    pub name: &'a str,
    pub grfid: u32,
    pub status: LoadStatus,
    pub reserved: bool,
    pub errors: Vec<LoadDiagnostic>,
}
pub(super) struct Registry<'a> {
    pub configs: Vec<Config<'a>>,
    pub files: Vec<File<'a>>,
}

impl<'a> Registry<'a> {
    pub(super) fn new(inputs: &[LoadInput<'a>]) -> Self {
        Self {
            configs: inputs
                .iter()
                .map(|input| Config {
                    name: input.name,
                    grfid: input.identity.grfid,
                    status: LoadStatus::Unknown,
                    reserved: false,
                    errors: Vec::new(),
                })
                .collect(),
            files: Vec::new(),
        }
    }
    pub(super) fn initialize(&mut self, input: LoadInput<'a>) {
        if !self.files.iter().any(|file| file.name == input.name) {
            self.files.push(File {
                cargo: None,
                language_maps: std::collections::BTreeMap::new(),
                features: 0,
                globals: super::load_context::FileGlobals::default(),
                name: input.name,
                grfid: input.identity.grfid,
                version: 0,
                parameters: input.parameters.to_vec(),
                labels: Vec::new(),
            });
        }
    }
    pub(super) fn file_index(&self, config: usize) -> Option<usize> {
        let name = self.configs.get(config)?.name;
        self.files.iter().position(|file| file.name == name)
    }
    pub(super) fn file(&self, config: usize) -> Option<&File<'a>> {
        self.file_index(config)
            .and_then(|file| self.files.get(file))
    }
    pub(super) fn file_mut(&mut self, config: usize) -> Option<&mut File<'a>> {
        let file = self.file_index(config)?;
        self.files.get_mut(file)
    }
    pub(super) fn config_by_id(&self, id: u32, mask: u32) -> Option<usize> {
        self.configs
            .iter()
            .position(|config| config.grfid & mask == id & mask)
    }
    pub(super) fn file_by_id(&self, id: u32) -> Option<&File<'a>> {
        self.files.iter().find(|file| file.grfid == id)
    }
    pub(super) fn status(&self, config: usize) -> LoadStatus {
        self.configs
            .get(config)
            .map_or(LoadStatus::NotFound, |config| config.status)
    }
    pub(super) fn disable(
        &mut self,
        target: usize,
        current: usize,
        line: u32,
        failure: Option<LoadFailure>,
    ) {
        let file = if target == current {
            self.file_index(target)
        } else {
            self.configs
                .get(target)
                .and_then(|c| self.files.iter().position(|f| f.grfid == c.grfid))
        };
        if let Some(file) = file.and_then(|index| self.files.get_mut(index)) {
            file.labels.clear();
        }
        if let Some(config) = self.configs.get_mut(target) {
            config.status = LoadStatus::Disabled;
            if let Some(failure) = failure {
                if !config.errors.iter().any(|error| error.line == line) {
                    config.errors.push(LoadDiagnostic { failure, line });
                }
            }
        }
    }
    pub(super) fn snapshot_bytes(&self) -> usize {
        self.configs
            .iter()
            .enumerate()
            .fold(0_usize, |size, (index, config)| {
                size.saturating_add(std::mem::size_of::<FileControlState>())
                    .saturating_add(
                        config
                            .errors
                            .len()
                            .saturating_mul(std::mem::size_of::<LoadDiagnostic>()),
                    )
                    .saturating_add(self.file(index).map_or(0, |file| {
                        file.parameters.len().saturating_mul(4).saturating_add(
                            file.labels
                                .len()
                                .saturating_mul(std::mem::size_of::<LoadLabel>()),
                        )
                    }))
            })
    }
    pub(super) fn snapshots(&self) -> Vec<FileControlState> {
        self.configs
            .iter()
            .enumerate()
            .map(|(index, config)| {
                let file = self.file(index);
                FileControlState {
                    config_grfid: config.grfid,
                    file_grfid: file.map(|f| f.grfid),
                    version: file.map(|f| f.version),
                    status: config.status,
                    reserved: config.reserved,
                    parameters: file.map(|f| f.parameters.clone()),
                    labels: file.map(|f| f.labels.clone()),
                    errors: config.errors.clone(),
                }
            })
            .collect()
    }
}
