FROM python:3.11-slim

WORKDIR /app

# Install dependencies with pinned OpenAI version
RUN pip install flask flask-cors gunicorn openai==0.28

COPY app.py .

CMD ["gunicorn", "-b", "0.0.0.0:5000", "app:app"]
