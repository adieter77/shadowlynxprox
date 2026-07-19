FROM python:3.11-slim

WORKDIR /app

# Install dependencies for local models
RUN pip install flask flask-cors gunicorn transformers accelerate torch

COPY app.py .

CMD ["gunicorn", "-b", "0.0.0.0:5000", "app:app"]
