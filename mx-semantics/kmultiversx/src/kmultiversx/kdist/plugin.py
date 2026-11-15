from __future__ import annotations

import shutil
from pathlib import Path
from typing import TYPE_CHECKING

from pyk.kbuild.utils import k_version
from pyk.kdist.api import Target
from pyk.ktool.kompile import PykBackend, kompile

if TYPE_CHECKING:
    from collections.abc import Callable, Mapping
    from typing import Any, Final


class SourceTarget(Target):
    SRC_DIR: Final = Path(__file__).parent

    def build(self, output_dir: Path, deps: dict[str, Path], args: dict[str, Any], verbose: bool) -> None:
        # wasm-semantics is a vendored copy, see wasm-semantics/VENDORED.md
        shutil.copytree(self.SRC_DIR / 'wasm-semantics', output_dir / 'wasm-semantics')
        shutil.copytree(self.SRC_DIR / 'krypto', output_dir / 'krypto')
        shutil.copytree(self.SRC_DIR / 'mx-semantics', output_dir / 'mx-semantics')

    def source(self) -> tuple[Path, ...]:
        return (self.SRC_DIR,)


class KompileTarget(Target):
    _kompile_args: Callable[[Path], Mapping[str, Any]]

    def __init__(self, kompile_args: Callable[[Path], Mapping[str, Any]]):
        self._kompile_args = kompile_args

    def build(self, output_dir: Path, deps: dict[str, Path], args: dict[str, Any], verbose: bool) -> None:
        # TODO Pass K_OPTS='-Xmx8G -Xss512m'
        kompile_args = self._kompile_args(deps['mx-semantics.source'])
        kompile(output_dir=output_dir, verbose=verbose, **kompile_args)

    def context(self) -> dict[str, str]:
        return {'k-version': k_version().text}

    def deps(self) -> tuple[str]:
        return ('mx-semantics.source',)


def llvm_target(main_file_name: str, main_module: str, syntax_module: str) -> KompileTarget:
    return KompileTarget(
        lambda src_dir: {
            'backend': PykBackend.LLVM,
            'main_file': src_dir / 'mx-semantics' / main_file_name,
            'main_module': main_module,
            'syntax_module': syntax_module,
            'include_dirs': [src_dir],
            'md_selector': 'k',
            'hook_namespaces': ['KRYPTO'],
            'opt_level': 2,
            # The KRYPTO hooks are compiled into the interpreter from source.
            'ccopts': ['-g', '-std=c++17', str(src_dir / 'krypto' / 'krypto.cpp'), '-lcrypto'],
        },
    )


def haskell_target(main_file_name: str, main_module: str, syntax_module: str) -> KompileTarget:
    return KompileTarget(
        lambda src_dir: {
            'backend': PykBackend.HASKELL,
            'main_file': src_dir / 'mx-semantics' / main_file_name,
            'main_module': main_module,
            'syntax_module': syntax_module,
            'include_dirs': [src_dir],
            'md_selector': 'k',
            'hook_namespaces': ['KRYPTO'],
            'warnings_to_errors': True,
        },
    )


__TARGETS__: Final = {
    'source': SourceTarget(),
    'llvm-mandos': llvm_target('mandos.md', 'MANDOS', 'MANDOS-SYNTAX'),
    'llvm-kasmer': llvm_target('kasmer.md', 'KASMER', 'KASMER-SYNTAX'),
    'haskell-mandos': haskell_target('mandos.md', 'MANDOS', 'MANDOS-SYNTAX'),
    'haskell-kasmer': haskell_target('kasmer.md', 'KASMER', 'KASMER-SYNTAX'),
}
