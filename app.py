from flask import Flask, request, jsonify
from flask_cors import CORS

app = Flask(__name__)
CORS(app, origins=["https://adieter77.github.io"])  # allow GitHub Pages frontend

@app.route("/chat", methods=["POST"])
def chat():
    data = request.json
    user_msg = data.get("message", "")

    # Simple placeholder AI logic (replace with real AI later)
    if user_msg.strip().lower() == "hello":
        ai_reply = "Hi there! How can I help you today?"
    else:
        ai_reply = f"I received your message: {user_msg}"

    return jsonify({"reply": ai_reply})

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
