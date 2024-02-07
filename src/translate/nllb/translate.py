from transformers import AutoTokenizer, AutoModelForSeq2SeqLM, pipeline

model_name = "facebook/nllb-200-3.3B"
model = AutoModelForSeq2SeqLM.from_pretrained(model_name)
tokenizer = AutoTokenizer.from_pretrained(model_name)
translator = pipeline('translation', model=model, tokenizer=tokenizer, src_lang="jpn_Jpan", tgt_lang="eng_Latn")

def translate(line):
    return translator(line, max_length=200)[0]['translation_text']
