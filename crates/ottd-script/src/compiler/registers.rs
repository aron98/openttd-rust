//! Native local/temporary register lifetime and target-stack ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Register(pub u8);
#[derive(Clone, Copy)]
enum Slot<'a> {
    Root,
    Local(&'a str),
    Temporary,
}
pub(super) struct Registers<'a> {
    slots: Vec<Slot<'a>>,
    targets: Vec<Register>,
    pub high_water: u16,
}
impl<'a> Registers<'a> {
    pub(super) fn new() -> Self {
        Self {
            slots: vec![Slot::Root],
            targets: Vec::new(),
            high_water: 1,
        }
    }
    pub(super) fn size(&self) -> Option<u8> {
        u8::try_from(self.slots.len()).ok()
    }
    pub(super) fn local(&self, name: &str) -> Option<Register> {
        self.slots
            .iter()
            .rposition(|slot| matches!(slot, Slot::Local(found) if *found == name))
            .and_then(|index| u8::try_from(index).ok())
            .map(Register)
    }
    pub(super) fn is_local(&self, register: u8) -> bool {
        matches!(
            self.slots.get(usize::from(register)),
            Some(Slot::Local(_) | Slot::Root)
        )
    }
    fn allocate(&mut self, slot: Slot<'a>) -> Option<Register> {
        let index = self.size()?;
        if index == 255 {
            return None;
        }
        self.slots.push(slot);
        self.high_water = self.high_water.max(u16::from(index).checked_add(1)?);
        Some(Register(index))
    }
    pub(super) fn push(&mut self) -> Option<Register> {
        let register = self.allocate(Slot::Temporary)?;
        self.targets.push(register);
        Some(register)
    }
    pub(super) fn reference(&mut self, register: Register) {
        self.targets.push(register);
    }
    pub(super) fn pop(&mut self) -> Option<Register> {
        let register = self.targets.pop()?;
        match self.slots.get(usize::from(register.0))? {
            Slot::Temporary => {
                let _slot = self.slots.pop();
            }
            Slot::Local(_) | Slot::Root => {}
        }
        Some(register)
    }
    pub(super) fn bind(&mut self, name: &'a str) -> Option<Register> {
        self.allocate(Slot::Local(name))
    }
    pub(super) fn truncate(&mut self, size: u8) {
        self.slots.truncate(usize::from(size));
    }
}
