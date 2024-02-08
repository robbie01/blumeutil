from dataclasses import dataclass
from typing import Optional

import sqlite3
db = sqlite3.connect("../blume.db")

from exllamav2 import(
    ExLlamaV2,
    ExLlamaV2Config,
    ExLlamaV2Cache,
    ExLlamaV2Tokenizer
)

from exllamav2.generator import (
    ExLlamaV2BaseGenerator,
    ExLlamaV2Sampler
)

model_dir = "/home/robbie/sdiff/exllamav2/vntl-7b-v0.3.1-hf-8.0bpw-exl2"

config = ExLlamaV2Config()
config.model_dir = model_dir
config.prepare()

model = ExLlamaV2(config)

cache = ExLlamaV2Cache(model, lazy = True)
model.load_autosplit(cache)

tokenizer = ExLlamaV2Tokenizer(config)

# Initialize generator

generator = ExLlamaV2BaseGenerator(model, cache, tokenizer)
generator.warmup()

settings = ExLlamaV2Sampler.Settings()
settings.temperature = 0.6
#settings.top_k = 1
settings.top_p = 0.9

session = 'vntl-20240206'

scriptids = [id for id, in db.cursor().execute("""
    SELECT DISTINCT scriptid
    FROM lines
    WHERE (?, scriptid, address)
        NOT IN (SELECT session, scriptid, address FROM translations)
""", (session,))]

@dataclass
class CharacterAlias:
    jp: str
    en: str

@dataclass
class Character:
    jp_short: str
    en_short: str
    gender: Optional[str] = None
    aliases: list[CharacterAlias] = []

characters = [
    Character(
        '？？？', '???'
    ),
    Character(
        'メアリ', 'Mary', 'Female'
    ),
    Character(
        'ダニエラ', 'Daniela', 'Female',
        [CharacterAlias('ダニエラ・ブランクーシ', 'Daniela Brancusi')]
    ),
    Character(
        'ヴィクトル', 'Victor', 'Male',
        [CharacterAlias('ヴィクトル・フリードリヒ', 'Victor Friedrich')]
    ),
    Character(
        'オーギュスト', 'Auguste', 'Male',
        [CharacterAlias('オーギュスト・ミュラー', 'Auguste Mueller')]
    ),
    Character(
        'イリヤ', 'Ilya', 'Female',
        [CharacterAlias('イリヤ・カンテミール', 'Ilya Cantemir')]
    ),
    Character(
        'リチャード', 'Richard', 'Male',
        [
            CharacterAlias('リチャード・カンテミール', 'Richard Cantemir'),
            CharacterAlias('お兄ちゃん', 'Onii-chan') # rip
        ]
    ),
    Character(
        'ヤコブ', 'Jacob', 'Male',
        [
            CharacterAlias('ヤコブ・カンテミール', 'Jacob Cantemir'),
            CharacterAlias('村長', 'mayor')
        ]
    ),
    Character(
        'オーギュストの声', 'Auguste\'s voice'
    ),
    Character(
        'バージニア', 'Virginia', 'Female',
        [CharacterAlias('バージニア・モレノ', 'Virginia Moreno')]
    ),
    Character(
        'クラウス', 'Klaus', 'Male'
    ),
    Character(
        'ステファン', 'Stefan', 'Male'
    ),
    Character(
        'レルム', 'Relm', 'Male'
    ),
    Character(
        'ジェラルド', 'Gerald', 'Male'
    ),
    Character(
        'レオ', 'Leo', 'Male'
    ),
    Character(
        'ギルベルト', 'Gilbert', 'Male'
    ),
    Character(
        'バージニアの声', 'Virginia\'s voice'
    )
]

def get_character(jp_short: str):
    ch = next((ch for ch in characters if ch.jp_short == jp_short), None)
    if ch is None:
        raise KeyError(jp_short)
    return ch

def gen_header(chars: set[str]):
    header = '<s><<START>>\n'
    for c in chars:
        ch = get_character(c)
        if ch.gender is None:
            continue
        full = ch[0] if ch.aliases else CharacterAlias(ch.jp_short, ch.en_short)
        header += "Name: {} ({}) | Gender: {}".format(full.en, full.jp, ch.gender)
        if len(ch.aliases) > 1:
            header += " | Aliases: {}".format(", ".join("{} ({})".format(a.en, a.jp) for a in ch.aliases[1:]))
        header += '\n'
    return header

def do_script(scriptid):
    chars = set()
    seen = []

    for address, c, line, translation in db.cursor().execute("""
        SELECT lines.address, speaker, line, translation
        FROM lines LEFT JOIN translations ON
            lines.scriptid = translations.scriptid AND
            lines.address = translations.address AND
            translations.session = ?
        WHERE lines.scriptid = ?
    """, (session, scriptid)):
        if c:
            ch = get_character(c)
            chars.add(c)
            jpspeaker = '[{}]: '.format(c)
            enspeaker = '[{}]: '.format(ch.en_short)
        else:
            jpspeaker = ''
            enspeaker = ''

        if translation:
            seen.append("<<JAPANESE>>\n{}{}\n<<ENGLISH>> (fidelity = high)\n{}{}\n".format(jpspeaker, line, enspeaker, translation))
            continue
        
        nucleus = "<<JAPANESE>\n{}{}\n<<ENGLISH>> (fidelity = high)\n{}".format(jpspeaker, line, enspeaker)