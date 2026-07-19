from flask import Flask, request, jsonify
from flask_cors import CORS
import requests

app = Flask(__name__)
CORS(app, origins=["https://adieter77.github.io"])  # allow GitHub Pages frontend

AI_CORE_URL = "http://localhost:8000/chat"  # replace with your AI core service URL

@app.route("/chat", methods=["POST"])
def chat():
    data = request.json
    user_msg = data.get("message", "")

    try:
        # Forward message to AI core service
        res = requests.post(AI_CORE_URL, json={"message": user_msg})
        ai_reply = res.json().get("reply", "No reply from AI core")
    except Exception as e:
        ai_reply = f"Error contacting AI core: {str(e)}"

    return jsonify({"reply": ai_reply})

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
