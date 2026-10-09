//! Channel layouts and their ordered speaker positions.

/// Channel formats (track widths).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum ChannelFormat {
    #[default]
    Mono,
    Stereo,
    Lcr,
    Quad,
    Lcrs,
    Surround50,
    Surround51,
    Surround60,
    Surround61,
    Sdds70,
    Sdds71,
    Surround70,
    Surround71,
    Atmos702,
    Atmos712,
    Atmos502,
    Atmos512,
    Atmos504,
    Atmos514,
    Atmos704,
    Atmos714,
    Atmos706,
    Atmos716,
    Atmos904,
    Atmos914,
    Atmos906,
    Atmos916,
    /// Ambisonics of order 1..=7 ((n+1)² channels).
    Ambisonics(u8),
}

impl ChannelFormat {
    pub const ALL: [ChannelFormat; 34] = [
        ChannelFormat::Mono,
        ChannelFormat::Stereo,
        ChannelFormat::Lcr,
        ChannelFormat::Quad,
        ChannelFormat::Lcrs,
        ChannelFormat::Surround50,
        ChannelFormat::Surround51,
        ChannelFormat::Surround60,
        ChannelFormat::Surround61,
        ChannelFormat::Sdds70,
        ChannelFormat::Sdds71,
        ChannelFormat::Surround70,
        ChannelFormat::Surround71,
        ChannelFormat::Atmos702,
        ChannelFormat::Atmos712,
        ChannelFormat::Ambisonics(1),
        ChannelFormat::Ambisonics(2),
        ChannelFormat::Ambisonics(3),
        ChannelFormat::Ambisonics(4),
        ChannelFormat::Ambisonics(5),
        ChannelFormat::Ambisonics(6),
        ChannelFormat::Ambisonics(7),
        ChannelFormat::Atmos502,
        ChannelFormat::Atmos512,
        ChannelFormat::Atmos504,
        ChannelFormat::Atmos514,
        ChannelFormat::Atmos704,
        ChannelFormat::Atmos714,
        ChannelFormat::Atmos706,
        ChannelFormat::Atmos716,
        ChannelFormat::Atmos904,
        ChannelFormat::Atmos914,
        ChannelFormat::Atmos906,
        ChannelFormat::Atmos916,
    ];
    pub fn channels(self) -> usize {
        match self {
            ChannelFormat::Mono => 1,
            ChannelFormat::Stereo => 2,
            ChannelFormat::Lcr => 3,
            ChannelFormat::Quad | ChannelFormat::Lcrs => 4,
            ChannelFormat::Surround50 => 5,
            ChannelFormat::Surround51 | ChannelFormat::Surround60 => 6,
            ChannelFormat::Surround61 | ChannelFormat::Sdds70 | ChannelFormat::Surround70 | ChannelFormat::Atmos502 => 7,
            ChannelFormat::Sdds71 | ChannelFormat::Surround71 | ChannelFormat::Atmos512 => 8,
            ChannelFormat::Atmos702 | ChannelFormat::Atmos504 => 9,
            ChannelFormat::Atmos712 | ChannelFormat::Atmos514 => 10,
            ChannelFormat::Atmos704 => 11,
            ChannelFormat::Atmos714 => 12,
            ChannelFormat::Atmos706 | ChannelFormat::Atmos904 => 13,
            ChannelFormat::Atmos716 | ChannelFormat::Atmos914 => 14,
            ChannelFormat::Atmos906 => 15,
            ChannelFormat::Atmos916 => 16,
            ChannelFormat::Ambisonics(n) => {
                let n = usize::from(n.clamp(1, 7)) + 1;
                n * n
            }
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ChannelFormat::Mono => "Mono",
            ChannelFormat::Stereo => "Stereo",
            ChannelFormat::Lcr => "LCR",
            ChannelFormat::Quad => "Quad",
            ChannelFormat::Lcrs => "LCRS",
            ChannelFormat::Surround50 => "5.0",
            ChannelFormat::Surround51 => "5.1",
            ChannelFormat::Surround60 => "6.0",
            ChannelFormat::Surround61 => "6.1",
            ChannelFormat::Sdds70 => "7.0 SDDS",
            ChannelFormat::Sdds71 => "7.1 SDDS",
            ChannelFormat::Surround70 => "7.0",
            ChannelFormat::Surround71 => "7.1",
            ChannelFormat::Atmos702 => "7.0.2",
            ChannelFormat::Atmos712 => "7.1.2",
            ChannelFormat::Atmos502 => "5.0.2",
            ChannelFormat::Atmos512 => "5.1.2",
            ChannelFormat::Atmos504 => "5.0.4",
            ChannelFormat::Atmos514 => "5.1.4",
            ChannelFormat::Atmos704 => "7.0.4",
            ChannelFormat::Atmos714 => "7.1.4",
            ChannelFormat::Atmos706 => "7.0.6",
            ChannelFormat::Atmos716 => "7.1.6",
            ChannelFormat::Atmos904 => "9.0.4",
            ChannelFormat::Atmos914 => "9.1.4",
            ChannelFormat::Atmos906 => "9.0.6",
            ChannelFormat::Atmos916 => "9.1.6",
            ChannelFormat::Ambisonics(1) => "1st Order Ambisonics",
            ChannelFormat::Ambisonics(2) => "2nd Order Ambisonics",
            ChannelFormat::Ambisonics(3) => "3rd Order Ambisonics",
            ChannelFormat::Ambisonics(4) => "4th Order Ambisonics",
            ChannelFormat::Ambisonics(5) => "5th Order Ambisonics",
            ChannelFormat::Ambisonics(6) => "6th Order Ambisonics",
            ChannelFormat::Ambisonics(_) => "7th Order Ambisonics",
        }
    }
    pub fn from_id(s: &str) -> Option<ChannelFormat> {
        ChannelFormat::ALL.into_iter().find(|f| f.label().eq_ignore_ascii_case(s.trim()))
    }
    /// Speakers in channel order, or empty for Ambisonics (ACN order, SN3D).
    ///
    /// Channel order is SMPTE / WAV order: speakers with a WAVE_FORMAT_EXTENSIBLE mask bit come in
    /// ascending bit order (L R C LFE Ls Rs Lc Rc Cs Lss Rss Ltf Rtf Ltr Rtr; 7.x puts the rear
    /// surrounds Lrs/Rrs on the "back" bits before the sides Lss/Rss), followed by speakers WAV
    /// cannot name (the top-middle pair of .6 layouts). E.g. 5.1 = L R C LFE Ls Rs,
    /// 7.1 = L R C LFE Lrs Rrs Lss Rss, 7.1.4 = L R C LFE Lrs Rrs Lss Rss Ltf Rtf Ltr Rtr.
    pub fn speakers(self) -> &'static [Speaker] {
        use Speaker::*;
        match self {
            ChannelFormat::Mono => &[C],
            ChannelFormat::Stereo => &[L, R],
            ChannelFormat::Lcr => &[L, R, C],
            ChannelFormat::Quad => &[L, R, Ls, Rs],
            ChannelFormat::Lcrs => &[L, R, C, Cs],
            ChannelFormat::Surround50 => &[L, R, C, Ls, Rs],
            ChannelFormat::Surround51 => &[L, R, C, Lfe, Ls, Rs],
            ChannelFormat::Surround60 => &[L, R, C, Ls, Rs, Cs],
            ChannelFormat::Surround61 => &[L, R, C, Lfe, Ls, Rs, Cs],
            ChannelFormat::Sdds70 => &[L, R, C, Ls, Rs, Lc, Rc],
            ChannelFormat::Sdds71 => &[L, R, C, Lfe, Ls, Rs, Lc, Rc],
            ChannelFormat::Surround70 => &[L, R, C, Lrs, Rrs, Lss, Rss],
            ChannelFormat::Surround71 => &[L, R, C, Lfe, Lrs, Rrs, Lss, Rss],
            ChannelFormat::Atmos702 => &[L, R, C, Lrs, Rrs, Lss, Rss, Ltm, Rtm],
            ChannelFormat::Atmos712 => &[L, R, C, Lfe, Lrs, Rrs, Lss, Rss, Ltm, Rtm],
            ChannelFormat::Atmos502 => &[L, R, C, Ls, Rs, Ltm, Rtm],
            ChannelFormat::Atmos512 => &[L, R, C, Lfe, Ls, Rs, Ltm, Rtm],
            ChannelFormat::Atmos504 => &[L, R, C, Ls, Rs, Ltf, Rtf, Ltr, Rtr],
            ChannelFormat::Atmos514 => &[L, R, C, Lfe, Ls, Rs, Ltf, Rtf, Ltr, Rtr],
            ChannelFormat::Atmos704 => &[L, R, C, Lrs, Rrs, Lss, Rss, Ltf, Rtf, Ltr, Rtr],
            ChannelFormat::Atmos714 => &[L, R, C, Lfe, Lrs, Rrs, Lss, Rss, Ltf, Rtf, Ltr, Rtr],
            ChannelFormat::Atmos706 => &[L, R, C, Lrs, Rrs, Lss, Rss, Ltf, Rtf, Ltr, Rtr, Ltm, Rtm],
            ChannelFormat::Atmos716 => &[L, R, C, Lfe, Lrs, Rrs, Lss, Rss, Ltf, Rtf, Ltr, Rtr, Ltm, Rtm],
            ChannelFormat::Atmos904 => &[L, R, C, Lrs, Rrs, Lw, Rw, Lss, Rss, Ltf, Rtf, Ltr, Rtr],
            ChannelFormat::Atmos914 => &[L, R, C, Lfe, Lrs, Rrs, Lw, Rw, Lss, Rss, Ltf, Rtf, Ltr, Rtr],
            ChannelFormat::Atmos906 => &[L, R, C, Lrs, Rrs, Lw, Rw, Lss, Rss, Ltf, Rtf, Ltr, Rtr, Ltm, Rtm],
            ChannelFormat::Atmos916 => &[L, R, C, Lfe, Lrs, Rrs, Lw, Rw, Lss, Rss, Ltf, Rtf, Ltr, Rtr, Ltm, Rtm],
            ChannelFormat::Ambisonics(_) => &[],
        }
    }
    pub fn is_ambisonic(self) -> bool {
        matches!(self, ChannelFormat::Ambisonics(_))
    }
    pub fn has_lfe(self) -> bool {
        self.speakers().contains(&Speaker::Lfe)
    }
    pub fn has_height(self) -> bool {
        self.speakers().iter().any(|s| s.is_height())
    }
    /// Short per-channel labels for meters (`W Y Z X 5…` for Ambisonics).
    pub fn channel_label(self, ch: usize) -> String {
        match self.speakers().get(ch) {
            Some(s) => s.label().to_string(),
            None if self.is_ambisonic() => ["W", "Y", "Z", "X"].get(ch).map_or_else(|| format!("{}", ch + 1), |s| s.to_string()),
            None => format!("{}", ch + 1),
        }
    }
    /// WAVE_FORMAT_EXTENSIBLE channel mask (0 = no positions, e.g. Ambisonics). Speakers that WAV
    /// cannot name (top middle) borrow the top-front bits when the layout has no top fronts.
    pub fn channel_mask(self) -> u32 {
        let sp = self.speakers();
        let has_tf = sp.contains(&Speaker::Ltf);
        sp.iter().fold(0u32, |m, s| match s {
            Speaker::Ltm if !has_tf => m | 0x1000,
            Speaker::Rtm if !has_tf => m | 0x4000,
            s => m | s.wave_bit(),
        })
    }
    /// Best format for a channel count.
    pub fn for_channels(n: usize) -> ChannelFormat {
        [
            ChannelFormat::Mono,
            ChannelFormat::Stereo,
            ChannelFormat::Lcr,
            ChannelFormat::Quad,
            ChannelFormat::Surround50,
            ChannelFormat::Surround51,
            ChannelFormat::Surround70,
            ChannelFormat::Surround71,
        ]
        .into_iter()
        .find(|f| f.channels() == n)
        .unwrap_or(if n <= 1 { ChannelFormat::Mono } else { ChannelFormat::Stereo })
    }
}

/// A loudspeaker position in a channel format.
///
/// Angles follow ITU-R BS.775 / Dolby conventions: azimuth in degrees, 0 = front centre, positive
/// = to the right; elevation in degrees above the listener plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Speaker {
    L,
    R,
    C,
    Lfe,
    /// 5.x surrounds (±110°).
    Ls,
    Rs,
    /// SDDS inner fronts (±15°).
    Lc,
    Rc,
    /// Centre surround (180°).
    Cs,
    /// 7.x side surrounds (±90°).
    Lss,
    Rss,
    /// 7.x rear surrounds (±150°).
    Lrs,
    Rrs,
    /// 9.x front wides (±60°).
    Lw,
    Rw,
    /// Top front (±45°, elevation 45°).
    Ltf,
    Rtf,
    /// Top middle (±90°, elevation 45°).
    Ltm,
    Rtm,
    /// Top rear (±135°, elevation 45°).
    Ltr,
    Rtr,
}

impl Speaker {
    pub fn azimuth(self) -> f32 {
        match self {
            Speaker::L => -30.0,
            Speaker::R => 30.0,
            Speaker::C | Speaker::Lfe => 0.0,
            Speaker::Ls => -110.0,
            Speaker::Rs => 110.0,
            Speaker::Lc => -15.0,
            Speaker::Rc => 15.0,
            Speaker::Cs => 180.0,
            Speaker::Lss => -90.0,
            Speaker::Rss => 90.0,
            Speaker::Lrs => -150.0,
            Speaker::Rrs => 150.0,
            Speaker::Lw => -60.0,
            Speaker::Rw => 60.0,
            Speaker::Ltf => -45.0,
            Speaker::Rtf => 45.0,
            Speaker::Ltm => -90.0,
            Speaker::Rtm => 90.0,
            Speaker::Ltr => -135.0,
            Speaker::Rtr => 135.0,
        }
    }
    pub fn elevation(self) -> f32 {
        if self.is_height() { 45.0 } else { 0.0 }
    }
    pub fn is_height(self) -> bool {
        matches!(self, Speaker::Ltf | Speaker::Rtf | Speaker::Ltm | Speaker::Rtm | Speaker::Ltr | Speaker::Rtr)
    }
    pub fn is_lfe(self) -> bool {
        self == Speaker::Lfe
    }
    pub fn label(self) -> &'static str {
        match self {
            Speaker::L => "L",
            Speaker::R => "R",
            Speaker::C => "C",
            Speaker::Lfe => "LFE",
            Speaker::Ls => "Ls",
            Speaker::Rs => "Rs",
            Speaker::Lc => "Lc",
            Speaker::Rc => "Rc",
            Speaker::Cs => "Cs",
            Speaker::Lss => "Lss",
            Speaker::Rss => "Rss",
            Speaker::Lrs => "Lrs",
            Speaker::Rrs => "Rrs",
            Speaker::Lw => "Lw",
            Speaker::Rw => "Rw",
            Speaker::Ltf => "Ltf",
            Speaker::Rtf => "Rtf",
            Speaker::Ltm => "Ltm",
            Speaker::Rtm => "Rtm",
            Speaker::Ltr => "Ltr",
            Speaker::Rtr => "Rtr",
        }
    }
    /// WAVE_FORMAT_EXTENSIBLE `dwChannelMask` bit (0 for speakers WAV cannot name).
    pub fn wave_bit(self) -> u32 {
        match self {
            Speaker::L => 0x1,
            Speaker::R => 0x2,
            Speaker::C => 0x4,
            Speaker::Lfe => 0x8,
            Speaker::Ls | Speaker::Lrs => 0x10,
            Speaker::Rs | Speaker::Rrs => 0x20,
            Speaker::Lc | Speaker::Lw => 0x40,
            Speaker::Rc | Speaker::Rw => 0x80,
            Speaker::Cs => 0x100,
            Speaker::Lss => 0x200,
            Speaker::Rss => 0x400,
            Speaker::Ltf => 0x1000,
            Speaker::Rtf => 0x4000,
            Speaker::Ltr => 0x8000,
            Speaker::Rtr => 0x2_0000,
            Speaker::Ltm | Speaker::Rtm => 0,
        }
    }
}
