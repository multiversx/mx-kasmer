from pathlib import Path

import pytest

from kmxwasm.kasmer.utils import load_project

REPO_ROOT = (Path(__file__).parents[5]).resolve(strict=True)
PARAMETRIC_TESTS_DIR = REPO_ROOT / 'contracts' / 'parametric-tests'

PROJECT_DIRS = sorted(p.parent for p in PARAMETRIC_TESTS_DIR.glob('*/kasmer.json'))


@pytest.mark.parametrize('project_dir', PROJECT_DIRS, ids=lambda p: p.name)
def test_load_project(project_dir: Path) -> None:
    project = load_project(project_dir)
    # Every contract under test must resolve to exactly one compiled .wasm.
    for path in project.contract_paths():
        assert path.is_file()
