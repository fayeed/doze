//! Standard Windows dialog templates. Windows owns layout scaling and control rendering.
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, DS_CENTER, DS_MODALFRAME, DS_SETFONT, ES_AUTOVSCROLL, ES_MULTILINE,
    ES_NUMBER, ES_READONLY, WS_BORDER, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_POPUP, WS_SYSMENU,
    WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

pub(super) struct Template {
    bytes: Vec<u8>,
    controls: u16,
}

impl Template {
    pub fn new(title: &str, width: i16, height: i16) -> Self {
        let mut template = Self {
            bytes: Vec::new(),
            controls: 0,
        };
        template.dword(
            WS_POPUP.0
                | WS_CLIPCHILDREN.0
                | WS_CAPTION.0
                | WS_SYSMENU.0
                | DS_MODALFRAME as u32
                | DS_SETFONT as u32
                | DS_CENTER as u32,
        );
        template.dword(0);
        template.word(0); // Filled with the control count at finish.
        for value in [0, 0, width, height] {
            template.word(value as u16);
        }
        template.word(0); // No menu or custom window class.
        template.word(0);
        template.text(title);
        template.word(9);
        template.text("Segoe UI");
        template
    }

    fn word(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn dword(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn text(&mut self, text: &str) {
        for unit in text.encode_utf16().chain(Some(0)) {
            self.word(unit);
        }
    }
    fn align(&mut self) {
        while !self.bytes.len().is_multiple_of(4) {
            self.bytes.push(0);
        }
    }

    fn control(&mut self, id: u16, class: &str, text: &str, bounds: [i16; 4], style: u32) {
        self.align();
        self.dword(WS_CHILD.0 | WS_VISIBLE.0 | style);
        self.dword(0);
        for value in bounds {
            self.word(value as u16);
        }
        self.word(id);
        self.text(class);
        self.text(text);
        self.word(0);
        self.controls += 1;
    }
    pub fn label(&mut self, text: &str, x: i16, y: i16, width: i16) {
        self.label_with_id(0xffff, text, x, y, width);
    }
    pub fn label_with_id(&mut self, id: u16, text: &str, x: i16, y: i16, width: i16) {
        self.control(id, "STATIC", text, [x, y, width, 12], 0);
    }
    #[cfg(test)]
    pub fn checkbox_at(&mut self, id: u16, text: &str, bounds: [i16; 4]) {
        self.control(
            id,
            "BUTTON",
            text,
            bounds,
            WS_TABSTOP.0 | windows::Win32::UI::WindowsAndMessaging::BS_AUTOCHECKBOX as u32,
        );
    }
    pub fn read_only_text(&mut self, id: u16, bounds: [i16; 4]) {
        self.control(
            id,
            "EDIT",
            "",
            bounds,
            WS_TABSTOP.0
                | WS_BORDER.0
                | WS_VSCROLL.0
                | ES_MULTILINE as u32
                | ES_READONLY as u32
                | ES_AUTOVSCROLL as u32,
        );
    }
    pub fn edit(&mut self, id: u16, text: &str, x: i16, y: i16, width: i16) {
        self.control(
            id,
            "EDIT",
            text,
            [x, y, width, 14],
            WS_TABSTOP.0 | WS_BORDER.0 | ES_NUMBER as u32,
        );
    }
    pub fn date_time(&mut self, id: u16, x: i16, y: i16, width: i16, time: bool) {
        self.control(
            id,
            "SysDateTimePick32",
            "",
            [x, y, width, 16],
            WS_TABSTOP.0 | if time { 9 } else { 0 },
        );
    }
    pub fn button(&mut self, id: u16, text: &str, x: i16, y: i16, default: bool) {
        self.control(
            id,
            "BUTTON",
            text,
            [x, y, 62, 16],
            WS_TABSTOP.0 | if default { BS_DEFPUSHBUTTON as u32 } else { 0 },
        );
    }
    pub fn finish(mut self) -> Vec<u32> {
        self.bytes[8..10].copy_from_slice(&self.controls.to_le_bytes());
        self.align();
        self.bytes
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            .collect()
    }
}
