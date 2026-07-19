FROM python:3.11-slim

WORKDIR /app

# Only install lightweight dependencies
RUN pip install flask flask-cors gunicorn requests

COPY app.py .

CMD ["gunicorn", "-b", "0.0.0.0:5000", "app:app"]
