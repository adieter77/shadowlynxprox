import os
from flask import Flask, request, jsonify
from flask_cors import CORS
import openai

app = Flask(__name__)
CORS(app, origins=["https://adieter77.github.io"])  # allow GitHub Pages frontend

# Load your OpenAI API key from environment variable
openai.api_key = os.getenv("OPENAI_API_KEY")

@app.route("/chat", methods=["POST"])
def chat():
    data = request.json
    user_msg = data.get("message", "")

    try:
        # Call AI model for intelligent reply
        response = openai.ChatCompletion.create(
            model="gpt-4o-mini",
            messages=[{"role": "user", "content": user_msg}]
        )
        ai_reply = response.choices[0].message["content"]
    except Exception as e:
        ai_reply = f"Error generating AI reply: {str(e)}"

    return jsonify({"reply": ai_reply})

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
