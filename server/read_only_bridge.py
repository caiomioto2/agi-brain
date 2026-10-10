"""Glia graph reader. Private SSH stdin/stdout; no listener and no write operations.

Uses the existing GBrain PostgreSQL configuration, on its host only. Every query
runs in a PostgreSQL read-only transaction. Client input cannot select SQL.
"""
import json
import os
import pathlib
import subprocess
import sys
import urllib.parse


def literal(value):
    if not isinstance(value, str) or len(value) > 1000 or "\x00" in value:
        raise ValueError("Invalid page identity")
    return "'" + value.replace("'", "''") + "'"


ACTIVE = "p.deleted_at IS NULL AND NOT s.archived"
FROM = "pages p JOIN sources s ON s.id=p.source_id"
GRAPH = """
WITH active AS (
 SELECT p.id,p.slug,p.source_id AS source,p.type,COALESCE(p.title,'') AS title,
 COALESCE(NULLIF(p.frontmatter->>'created',''),p.created_at::text) AS created,
 p.updated_at::text AS updated FROM %s WHERE %s
), edges AS (
 SELECT l.from_page_id AS source,l.to_page_id AS target,l.link_type AS type
 FROM links l JOIN active a ON a.id=l.from_page_id JOIN active b ON b.id=l.to_page_id
)
SELECT json_build_object('generated_at',CURRENT_TIMESTAMP::text,
 'nodes',COALESCE((SELECT json_agg(active ORDER BY id) FROM active),'[]'::json),
 'links',COALESCE((SELECT json_agg(edges ORDER BY source,target,type) FROM edges),'[]'::json));
""" % (FROM, ACTIVE)


def query_for(request):
    op = request.get("op")
    if op == "graph":
        return GRAPH
    if op == "health":
        return "SELECT json_build_object('read_only',current_setting('transaction_read_only'),'database',current_database(),'protocol',1);"
    if op == "pages":
        keys = request.get("keys")
        if not isinstance(keys, list) or not 1 <= len(keys) <= 100:
            raise ValueError("Expected 1 to 100 page identities")
        predicates = []
        for key in keys:
            if not isinstance(key, dict):
                raise ValueError("Invalid page identity")
            predicates.append("(p.source_id=%s AND p.slug=%s)" % (literal(key.get("source")), literal(key.get("slug"))))
        return """SELECT COALESCE(json_agg(json_build_object('source',p.source_id,'slug',p.slug,
          'body',left(COALESCE(p.compiled_truth,'') || E'\\n\\n' || COALESCE(p.timeline,''),400000))), '[]'::json)
          FROM %s WHERE %s AND (%s);""" % (FROM, ACTIVE, " OR ".join(predicates))
    raise ValueError("Unsupported operation; this bridge only reads graph, pages or health")


def database_url():
    config = pathlib.Path(os.environ.get("GBRAIN_CONFIG", str(pathlib.Path.home() / ".gbrain/config.json")))
    data = json.loads(config.read_text())
    return os.environ.get("GBRAIN_DATABASE_URL") or os.environ.get("DATABASE_URL") or data.get("database_url")


def execute(sql):
    env = dict(os.environ)
    uri = database_url()
    if not uri:
        raise RuntimeError("Database is not configured")
    parsed = urllib.parse.urlsplit(uri)
    if parsed.scheme not in ("postgres", "postgresql") or not parsed.hostname:
        raise RuntimeError("PostgreSQL URI is required")
    # PGDATABASE does not expand an environment URI consistently in libpq.
    # Individual libpq variables keep passwords off command lines and stdout.
    env["PGHOST"] = parsed.hostname
    env["PGPORT"] = str(parsed.port or 5432)
    env["PGUSER"] = urllib.parse.unquote(parsed.username or "")
    env["PGPASSWORD"] = urllib.parse.unquote(parsed.password or "")
    env["PGDATABASE"] = urllib.parse.unquote(parsed.path.lstrip("/"))
    for key, value in urllib.parse.parse_qsl(parsed.query):
        if key in ("sslmode", "sslrootcert"):
            env["PG" + key.upper()] = value
    env["PGOPTIONS"] = "-c default_transaction_read_only=on -c statement_timeout=20000 -c search_path=public -c standard_conforming_strings=on"
    result = subprocess.run(["psql", "-X", "-q", "-t", "-A", "-v", "ON_ERROR_STOP=1"],
                            input="BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;\n" + sql + "\nCOMMIT;\n",
                            text=True, capture_output=True, env=env, timeout=25)
    if result.returncode:
        # Never relay driver errors containing connection credentials to the client.
        raise RuntimeError("Read-only database query failed")
    return json.loads(result.stdout)


def main():
    while True:
        line = sys.stdin.buffer.readline(65537)
        if not line:
            return
        request = {}
        try:
            if len(line) > 65536:
                raise ValueError("Request is too large")
            request = json.loads(line)
            result = execute(query_for(request))
            reply = {"id": request.get("id"), "result": result}
        except (ValueError, TypeError, AttributeError):
            reply = {"id": request.get("id") if isinstance(request, dict) else None, "error": "Invalid or unsupported read request"}
        except Exception:
            reply = {"id": request.get("id") if isinstance(request, dict) else None, "error": "Hosted reader unavailable; check server configuration"}
        print(json.dumps(reply, ensure_ascii=False, separators=(",", ":")), flush=True)


if __name__ == "__main__":
    main()
