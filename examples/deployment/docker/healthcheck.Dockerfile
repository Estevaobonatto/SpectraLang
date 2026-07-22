FROM spectralang:local

WORKDIR /app
COPY app.spectra /app/app.spectra

EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=2s --start-period=10s --retries=3 \
  CMD curl --fail --silent --show-error http://127.0.0.1:8080/healthz || exit 1

CMD ["spectralang", "run", "app.spectra"]
