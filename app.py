from flask import Flask, request, jsonify
from flask_cors import CORS

app = Flask(__name__)
CORS(app, origins=["https://adieter77.github.io"])  # allow GitHub Pages frontend

@app.route("/chat", methods=["POST"])
def chat():
    data = request.json
    user_msg = data.get("message", "")
    return jsonify({"reply": f"You said: {user_msg}"})

if __name__ == "__main__":
    app.run(host="0.0.0.0", port=5000)
