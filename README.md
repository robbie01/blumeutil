# Edel Blume translation utilities

These are confirmed to work with SLPM-66975 ([redump](http://redump.org/disc/53670/)) and SLPM-66976 ([redump](http://redump.org/disc/58880/)) (actually, the contents of both are identical. the executable just has a different name)

## Commands

- `uni`: analyze script.uni file (found in ISO filesystem). stores scripts in database as well as creates new file with patched scripts
- `stcm2`: analyze STCM2 scripts; specifically ones with ID in range [100, 199] as these contain dialogue. stores dialogue in database as well as patches scripts with new dialogue
- `translate`: create translation in database (supports Google Translate as well as OpenAI-compatible endpoint running `lmg-anon/vntl-llama3-8b-gguf`)
- `web`: web-based editor for translation
- `init`: initialize database
- `config`: set config option in database
- `cleanup`/`checkpunct`: various touch-ups

There are also two additional executables:

- `disasm`: experimental STCM2 disassembler
- `art`: yank art files from UNI2 files. confirmed to work with back.uni, chara.uni, etc.uni, memory.uni, and system.uni

## Acknowledgements

This project is eternally indebted to:

- UNI2 format: <https://mce.do.am/forum/25-36-1>
- STCM2 editor (only practical format description): <https://github.com/xyzz/hkki>
- most importantly: [lmg-anon](https://huggingface.co/lmg-anon) for their continual work on the VNTL models