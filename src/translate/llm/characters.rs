use std::{borrow::Cow, fmt::Display};
use once_cell::sync::Lazy;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Character {
    pub jpspeaker: &'static str,
    pub enspeaker: &'static str,
    pub gender: &'static str,
    pub jpfull: Option<&'static str>,
    pub enfull: Option<&'static str>,
    pub aliases: Box<[(&'static str, &'static str)]>
}

impl Display for Character {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Name: {} ({}) | Gender: {}",
            self.enfull.unwrap_or(self.enspeaker),
            self.jpfull.unwrap_or(self.jpspeaker),
            self.gender
        )?;

        if !self.aliases.is_empty() {
            f.write_str(" | Aliases: ")?;
            let mut aliases = self.aliases.iter().copied().peekable();
            while let Some((jp, en)) = aliases.next() {
                write!(f, "{en} ({jp})")?;
                if aliases.peek().is_some() {
                    f.write_str(", ")?;
                }
            }
        }

        Ok(())
    }
}

pub static CHARACTERS: Lazy<Box<[Character]>> = Lazy::new(|| Box::new([
    Character {
        jpspeaker: "メアリ",
        enspeaker: "Mary",
        gender: "Female",
        ..Default::default()
    },
    Character {
        jpspeaker: "ダニエラ",
        enspeaker: "Daniela",
        gender: "Female",
        jpfull: Some("ダニエラ・ブランクーシ"),
        enfull: Some("Daniela Brancusi"),
        ..Default::default()
    },
    Character {
        jpspeaker: "ヴィクトル",
        enspeaker: "Victor",
        gender: "Male",
        jpfull: Some("ヴィクトル・フリードリヒ"),
        enfull: Some("Victor Friedrich"),
        ..Default::default()
    },
    Character {
        jpspeaker: "オーギュスト",
        enspeaker: "Auguste",
        gender: "Male",
        jpfull: Some("オーギュスト・ミュラー"),
        enfull: Some("Auguste Mueller"),
        ..Default::default()
    },
    Character {
        jpspeaker: "イリヤ",
        enspeaker: "Ilya",
        gender: "Female",
        jpfull: Some("イリヤ・カンテミール"),
        enfull: Some("Ilya Cantemir"),
        ..Default::default()
    },
    Character {
        jpspeaker: "リチャード",
        enspeaker: "Richard",
        gender: "Male",
        jpfull: Some("リチャード・カンテミール"),
        enfull: Some("Richard Cantemir"),
        aliases: Box::new([("お兄ちゃん", "Onii-chan")]) // i'm so sorry
    },
    Character {
        jpspeaker: "ヤコブ",
        enspeaker: "Jacob",
        gender: "Male",
        jpfull: Some("ヤコブ・カンテミール"),
        enfull: Some("Jacob Cantemir"),
        aliases: Box::new([("村長", "mayor")])
    },
    Character {
        jpspeaker: "バージニア",
        enspeaker: "Virginia",
        gender: "Female",
        jpfull: Some("バージニア・モレノ"),
        enfull: Some("Virginia Moreno"),
        ..Default::default()
    },
    Character {
        jpspeaker: "ジェラルド",
        enspeaker: "Gerald",
        gender: "Male",
        jpfull: Some("ジェラルド・ヴィルベルヴィント"),
        enfull: Some("Gerald Villbervint"),
        ..Default::default()
    },
    Character {
        jpspeaker: "コンラッド",
        enspeaker: "Conrad",
        gender: "Male",
        jpfull: Some("コンラッド・バートリ"),
        enfull: Some("Conrad Bathory"),
        ..Default::default()
    },

    Character { jpspeaker: "クラウス", enspeaker: "Klaus", gender: "Male", ..Default::default() },
    Character { jpspeaker: "ステファン", enspeaker: "Stefan", gender: "Male", ..Default::default() },
    Character { jpspeaker: "レルム", enspeaker: "Relm", gender: "Male", ..Default::default() },
    Character { jpspeaker: "レオ", enspeaker: "Leo", gender: "Male", ..Default::default() },
    Character { jpspeaker: "ギルベルト", enspeaker: "Gilbert", gender: "Male", ..Default::default() },
]));

#[derive(Clone, Debug)]
pub enum EnSpeaker {
    Str(Cow<'static, str>),
    Character(&'static Character)
}

impl Display for EnSpeaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnSpeaker::Str(s) => s.fmt(f),
            EnSpeaker::Character(c) => c.enspeaker.fmt(f)
        }
    }
}

pub fn decode_jp_speaker(jpspeaker: &str) -> anyhow::Result<EnSpeaker> {
    if jpspeaker == "？？？" {
        return Ok(EnSpeaker::Str("???".into()));
    }
    for char in CHARACTERS.iter() {
        if char.jpspeaker == jpspeaker {
            return Ok(EnSpeaker::Character(char));
        }

        if jpspeaker.strip_prefix(char.jpspeaker).is_some_and(|s| s == "の声") {
            return Ok(EnSpeaker::Str((char.enspeaker.to_owned() + "'s voice").into()));
        }
    }
    Err(anyhow::anyhow!("bro I don't know {jpspeaker}"))
}