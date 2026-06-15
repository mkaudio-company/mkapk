use alloc::vec::Vec;

use crate::{Color, Pointf, Rectf, Sizef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ImageId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextLayoutId(pub u32);

#[derive(Debug, Clone, Copy)]
pub struct ColorStop {
    pub position: f32,
    pub color: Color,
}

#[derive(Debug, Clone, Copy)]
pub enum PaintCommand {
    Clear {
        color: Color,
    },
    FillRect {
        rect: Rectf,
        color: Color,
    },
    StrokeRect {
        rect: Rectf,
        color: Color,
        width: f32,
    },
    FillRoundedRect {
        rect: Rectf,
        radius: f32,
        color: Color,
    },
    StrokeRoundedRect {
        rect: Rectf,
        radius: f32,
        color: Color,
        width: f32,
    },
    FillPath {
        points: &'static [Pointf],
        color: Color,
    },
    StrokePath {
        points: &'static [Pointf],
        color: Color,
        width: f32,
    },
    LinearGradient {
        rect: Rectf,
        start: Pointf,
        end: Pointf,
        stops: &'static [ColorStop],
    },
    DrawImage {
        rect: Rectf,
        image: ImageId,
    },
    DrawText {
        position: Pointf,
        text: TextLayoutId,
    },
}

pub struct CommandList {
    commands: Vec<PaintCommand>,
}

impl CommandList {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            commands: Vec::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, cmd: PaintCommand) {
        self.commands.push(cmd);
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.commands.capacity()
    }

    pub fn iter(&self) -> core::slice::Iter<'_, PaintCommand> {
        self.commands.iter()
    }
}

impl Default for CommandList {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a CommandList {
    type Item = &'a PaintCommand;
    type IntoIter = core::slice::Iter<'a, PaintCommand>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub trait RenderBackend {
    fn begin(&mut self, _size: Sizef) {}
    fn replay(&mut self, _commands: &CommandList) {}
    fn end(&mut self) {}
}
