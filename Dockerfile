FROM python:3.11-slim

WORKDIR /app

# Install dependencies
RUN pip install flask flask-cors gunicorn

COPY app.py .

# Run with Gunicorn for production
CMD ["gunicorn", "-b", "0.0.0.0:5000", "app:app"]
