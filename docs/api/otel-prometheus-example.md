# Exemplo integrado OpenTelemetry + Prometheus

O exemplo `examples/api/10_otel_prometheus.spectra` demonstra a integração
real entre `std.api.trace` e o servidor HTTP da API.

O programa configura um exporter OTLP/HTTP, cria um span com atributos string,
inteiro e booleano, inicia uma rota HTTP real e mantém o processo ativo para o
harness enviar requests TCP. O `HttpServer` registra automaticamente as
métricas HTTP e expõe `GET /metrics` no formato Prometheus.

O fluxo certificado é executado por:

```powershell
python scripts\validate_r2707_exporters_example.py `
  --binary target\debug\spectralang.exe `
  --fixture tests\validation\200_otel_prometheus_example.spectra `
  --report target\r2707-otel-prometheus\report.json
```

O harness inicia um collector OTLP HTTP real em processo separado, executa o
fixture, envia requests para `/demo` e `/missing`, consulta `/metrics`,
decodifica o protobuf OTLP independentemente do runtime e valida o texto
Prometheus com parser independente. PostgreSQL e Redis não fazem parte deste
exemplo enquanto R-2505 e R-2507 permanecem incompletas.

O relatório versionado comprova spans `http.server`, atributos tipados,
flush, shutdown, métricas de sucesso/erro, histograma de duração, ausência de
segredos e encerramento dos processos auxiliares.
