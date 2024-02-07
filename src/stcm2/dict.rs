use std::collections::HashMap;
use once_cell::sync::Lazy;

pub static  SPEAKERS: Lazy<HashMap<Option<&str>, &str>> = Lazy::new(|| HashMap::from([
    (Some("？？？"), "???"), 
    (Some("イリヤ"), "Ilia"),
    (Some("リチャード"), "Richard"),
    (None, "Narrator"),
    (Some("ダニエラ"), "Daniela"),
    (Some("ヴィクトル"), "Victor"),
    (Some("#Name[1]"), "Mary"),
    (Some("オーギュスト"), "Auguste"),
    (Some("ヤコブ"), "Jacob"),
    (Some("オーギュストの声"), "Auguste's Voice"),
    (Some("バージニア"), "Virginia"),
    (Some("クラウス"), "Klaus"),
    (Some("ステファン"), "Stefan"),
    (Some("レルム"), "Relm"),
    (Some("ジェラルド"), "Gerald")
]));
