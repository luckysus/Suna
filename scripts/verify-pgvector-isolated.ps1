# Formal PostgreSQL/pgvector verification using Suna knowledge migrations + search SQL.
# Requires Docker. Uses container-local psql (no host psql required).
$ErrorActionPreference = "Stop"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$outDir = "F:/Suna/.suna-temp/pgvector-verify-round3-rework"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$log = Join-Path $outDir "run.log"
function Log([string]$msg) {
  $line = "$(Get-Date -Format o) $msg"
  Add-Content -Path $log -Value $line -Encoding utf8
  Write-Output $line
}

function Write-Status($obj) {
  $obj | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $outDir "status.json") -Encoding utf8
}

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
  Log "BLOCKED: docker CLI not found on PATH."
  Write-Status @{
    status = "blocked"
    reason = "docker_missing"
    outputDir = $outDir
    evidence = "Get-Command docker failed"
  }
  exit 2
}

$container = "suna-pgvector-round3-rework"
$port = 55433
$seed = 424242
$dimension = 8
$lists = 100
$image = "pgvector/pgvector:pg16"

docker rm -f $container 2>$null | Out-Null
Log "Pulling/starting $image on port $port"
try {
  docker run -d --name $container `
    -e POSTGRES_PASSWORD=suna_test `
    -e POSTGRES_USER=postgres `
    -e POSTGRES_DB=suna_knowledge `
    -p "${port}:5432" `
    $image | Out-Null
} catch {
  Log "BLOCKED: docker run failed: $_"
  Write-Status @{ status = "blocked"; reason = "docker_run_failed"; error = "$_"; outputDir = $outDir }
  exit 2
}

$ready = $false
for ($i = 0; $i -lt 40; $i++) {
  Start-Sleep -Seconds 2
  docker exec $container pg_isready -U postgres -d suna_knowledge 2>$null | Out-Null
  if ($LASTEXITCODE -eq 0) { $ready = $true; break }
}
if (-not $ready) {
  Log "BLOCKED: PostgreSQL did not become ready."
  docker logs $container 2>&1 | Set-Content (Join-Path $outDir "docker-logs.txt") -Encoding utf8
  Write-Status @{ status = "blocked"; reason = "postgres_not_ready"; outputDir = $outDir }
  exit 2
}

$migrationDir = "F:/Suna/src-tauri/migrations/knowledge"
$migrations = Get-ChildItem $migrationDir -Filter "*.sql" | Sort-Object Name
Log "Applying $($migrations.Count) formal knowledge migrations"
docker exec -i $container psql -U postgres -d suna_knowledge -v ON_ERROR_STOP=1 -c "CREATE EXTENSION IF NOT EXISTS vector;" | Tee-Object -FilePath (Join-Path $outDir "extension.log") | Out-Null
foreach ($file in $migrations) {
  Get-Content -Raw -Encoding utf8 $file.FullName | docker exec -i $container psql -U postgres -d suna_knowledge -v ON_ERROR_STOP=1 | Out-Null
  if ($LASTEXITCODE -ne 0) {
    Log "BLOCKED: migration failed: $($file.Name)"
    Write-Status @{ status = "blocked"; reason = "migration_failed"; file = $file.Name; outputDir = $outDir }
    exit 2
  }
  Log "Applied $($file.Name)"
}

$seedSql = @"
-- Synthetic formal-schema data (vector, not bytea). Seed=$seed dimension=$dimension
SELECT setseed(($seed % 1000)::float8 / 1000.0);
INSERT INTO knowledge_bases (id, name, description, created_at, updated_at)
VALUES ('11111111-1111-1111-1111-111111111111', 'synth-kb', 'isolated verify', now(), now())
ON CONFLICT DO NOTHING;

INSERT INTO source_documents (id, knowledge_base_id, source_path, content_sha256, display_name, source_kind, created_at, updated_at)
VALUES (
  '22222222-2222-2222-2222-222222222222',
  '11111111-1111-1111-1111-111111111111',
  'synth://doc',
  repeat('a', 64),
  'synth.pdf',
  'pdf',
  now(),
  now()
) ON CONFLICT DO NOTHING;

INSERT INTO document_versions (
  id, document_id, content_sha256, mime_type, parser, parser_version, chunk_policy_version,
  embedding_model_id, embedding_dimension, expected_asset_count, expected_chunk_count,
  manifest_sealed, activated_at, created_at
) VALUES (
  '33333333-3333-3333-3333-333333333333',
  '22222222-2222-2222-2222-222222222222',
  repeat('b', 64),
  'application/pdf',
  'synth',
  '1',
  '1',
  'synth-model',
  $dimension,
  0,
  32,
  TRUE,
  now(),
  now()
) ON CONFLICT DO NOTHING;

TRUNCATE document_chunks CASCADE;
TRUNCATE chunk_embeddings CASCADE;

INSERT INTO document_chunks (id, version_id, ordinal, title_path, text, source_location)
SELECT
  ('44444444-4444-4444-4444-' || lpad(g::text, 12, '0'))::uuid,
  '33333333-3333-3333-3333-333333333333',
  g,
  'chunk/' || g,
  'synthetic chunk text ' || g || ' steel yield 355',
  '{}'::jsonb
FROM generate_series(0, 31) AS g;

INSERT INTO chunk_embeddings (chunk_id, knowledge_base_id, document_id, model_id, dimension, embedding)
SELECT
  c.id,
  '11111111-1111-1111-1111-111111111111',
  '22222222-2222-2222-2222-222222222222',
  'synth-model',
  $dimension,
  (
    SELECT ('[' || string_agg((random())::text, ',') || ']')::vector
    FROM generate_series(1, $dimension)
  )
FROM document_chunks c;

CREATE INDEX IF NOT EXISTS chunk_embeddings_hnsw_$dimension
  ON chunk_embeddings
  USING hnsw ((embedding::vector($dimension)) vector_cosine_ops)
  WHERE dimension = $dimension;
"@

$seedSql | Set-Content (Join-Path $outDir "seed.sql") -Encoding utf8
Get-Content -Raw (Join-Path $outDir "seed.sql") | docker exec -i $container psql -U postgres -d suna_knowledge -v ON_ERROR_STOP=1 | Tee-Object -FilePath (Join-Path $outDir "seed.log") | Out-Null
if ($LASTEXITCODE -ne 0) {
  Log "BLOCKED: seed failed (schema may differ from insert assumptions)."
  Write-Status @{ status = "blocked"; reason = "seed_failed"; outputDir = $outDir; see = "seed.log" }
  exit 2
}

$indexSql = @"
SELECT indexname, indexdef
FROM pg_indexes
WHERE tablename = 'chunk_embeddings'
ORDER BY indexname;
"@
$indexSql | docker exec -i $container psql -U postgres -d suna_knowledge -v ON_ERROR_STOP=1 | Tee-Object -FilePath (Join-Path $outDir "indexes.txt") | Out-Null

$explainSql = @"
EXPLAIN (ANALYZE, BUFFERS)
SELECT c.id, (1 - ((e.embedding::vector($dimension)) <=> (
  SELECT embedding::vector($dimension) FROM chunk_embeddings LIMIT 1
)))::real AS rank
FROM chunk_embeddings e
JOIN document_chunks c ON c.id = e.chunk_id
JOIN document_versions v ON v.id = c.version_id AND v.activated_at IS NOT NULL
JOIN source_documents d ON d.id = v.document_id
JOIN knowledge_bases kb ON kb.id = d.knowledge_base_id AND kb.deleted_at IS NULL
WHERE d.knowledge_base_id = '11111111-1111-1111-1111-111111111111'
  AND d.deleted_at IS NULL
  AND e.knowledge_base_id = d.knowledge_base_id
  AND e.dimension = $dimension
  AND e.model_id = 'synth-model'
ORDER BY (e.embedding::vector($dimension)) <=> (
  SELECT embedding::vector($dimension) FROM chunk_embeddings LIMIT 1
), c.ordinal
LIMIT 10;
"@
$explainSql | Set-Content (Join-Path $outDir "explain.sql") -Encoding utf8
Get-Content -Raw (Join-Path $outDir "explain.sql") | docker exec -i $container psql -U postgres -d suna_knowledge -v ON_ERROR_STOP=1 | Tee-Object -FilePath (Join-Path $outDir "explain.txt") | Out-Null

$recallSql = @"
WITH query AS (
  SELECT embedding::vector($dimension) AS q FROM chunk_embeddings ORDER BY chunk_id LIMIT 1
),
exact AS (
  SELECT e.chunk_id,
         row_number() OVER (ORDER BY (e.embedding::vector($dimension)) <=> (SELECT q FROM query)) AS rn
  FROM chunk_embeddings e
  WHERE e.dimension = $dimension
),
ann AS (
  SELECT e.chunk_id,
         row_number() OVER (ORDER BY (e.embedding::vector($dimension)) <=> (SELECT q FROM query)) AS rn
  FROM chunk_embeddings e
  WHERE e.dimension = $dimension
  LIMIT 10
)
SELECT
  (SELECT count(*) FROM ann a JOIN exact x ON a.chunk_id = x.chunk_id AND x.rn <= 10)::float8 / 10.0 AS recall_at_10,
  (SELECT count(*) FROM ann) AS ann_count,
  (SELECT count(*) FROM exact WHERE rn <= 10) AS exact_count;
"@
$recallSql | Set-Content (Join-Path $outDir "recall.sql") -Encoding utf8
Get-Content -Raw (Join-Path $outDir "recall.sql") | docker exec -i $container psql -U postgres -d suna_knowledge -v ON_ERROR_STOP=1 | Tee-Object -FilePath (Join-Path $outDir "recall.txt") | Out-Null

$version = docker exec $container psql -U postgres -d suna_knowledge -tAc "SELECT version(); SELECT extversion FROM pg_extension WHERE extname='vector';"
$version | Set-Content (Join-Path $outDir "versions.txt") -Encoding utf8

$explainText = Get-Content -Raw (Join-Path $outDir "explain.txt")
$usedAnn = $explainText -match "hnsw|ivfflat"
Write-Status @{
  status = if ($usedAnn) { "ok" } else { "completed_without_ann_plan" }
  seed = $seed
  dimension = $dimension
  port = $port
  container = $container
  image = $image
  outputDir = $outDir
  ann_in_explain = [bool]$usedAnn
  artifacts = @("indexes.txt", "explain.txt", "recall.txt", "seed.sql", "versions.txt")
}
Log "Done. Artifacts in $outDir"
exit 0
