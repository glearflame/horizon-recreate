"""Smoke test: o L&L inicia e responde --help sem conta Microsoft nem recursos da Mojang."""
import subprocess, sys, os

CLIENT = os.environ.get("LL_CLIENT", os.path.expanduser("~/.local/bin/ll-client"))
RESOURCES = os.environ.get("LL_RESOURCES", os.path.expanduser("~/.horizon/resources"))

def test_client_help_runs():
    """O binário responde --help sem erro, sem precisar de conta ou recursos preparados."""
    out = subprocess.run([CLIENT, "--help"], capture_output=True, text=True, timeout=30)
    assert out.returncode == 0, f"--help falhou: {out.stderr}"
    assert "resources" in out.stdout, f"--help nao menciona resources: {out.stdout}"

def test_resources_dir_exists():
    """Os recursos preparados estao presentes (necessarios pra jogar)."""
    assert os.path.isdir(RESOURCES), f"resources nao encontrados em {RESOURCES}"

def test_no_microsoft_offline():
    """Nenhuma flag de auth Microsoft obrigatoria no help (jogo 100% offline)."""
    out = subprocess.run([CLIENT, "--help"], capture_output=True, text=True, timeout=30)
    help_text = out.stdout + out.stderr
    assert "microsoft" not in help_text.lower(), "help ainda menciona microsoft"
    assert "oauth" not in help_text.lower(), "help ainda menciona oauth"
