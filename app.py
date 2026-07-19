import requests
from flask import Flask, request, jsonify
from flask_cors import CORS

app = Flask(__name__)
CORS(app, origins=["https://adieter77.github.io"])

# Replace with the correct Jan.ai endpoint you confirmed above
JANAI_URL = "https://ether-rimless-cabbage.ngrok-free.dev/<correct-endpoint>"

@app.route("/chat", methods=["POST"])
def chat():
    data = request.json
    user_msg = data.get("message", "")

    try:
        response = requests.post(JANAI_URL, json={"prompt": user_msg})
        ai_reply = response.json().get("reply", "No reply from Jan.ai")
    except Exception as e:
        ai_reply = f"Error contacting Jan.ai: {str(e)}"

    return jsonify({"reply": ai_reply})

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
