from flask import Flask, request, jsonify
from flask_cors import CORS
from transformers import AutoTokenizer, AutoModelForCausalLM
import torch

app = Flask(__name__)
CORS(app, origins=["https://adieter77.github.io"])

# Load Venice Uncensored 1.2 model
MODEL_NAME = "gpt2"  # adjust if repo name differs
tokenizer = AutoTokenizer.from_pretrained(MODEL_NAME)
model = AutoModelForCausalLM.from_pretrained(
    MODEL_NAME,
    torch_dtype=torch.float16,
    device_map="auto"
)

@app.route("/chat", methods=["POST"])
def chat():
    data = request.json
    user_msg = data.get("message", "")

    # Encode input and generate reply
    inputs = tokenizer(user_msg, return_tensors="pt").to(model.device)
    outputs = model.generate(**inputs, max_length=200)
    ai_reply = tokenizer.decode(outputs[0], skip_special_tokens=True)

    return jsonify({"reply": ai_reply})

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
