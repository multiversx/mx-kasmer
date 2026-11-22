from __future__ import annotations

import logging
from typing import TYPE_CHECKING

from .utils import KasmerSetup

if TYPE_CHECKING:
    from typing import Final

    from .utils import KasmerProject


logger: Final = logging.getLogger(__name__)


def kasmer_claims(project: KasmerProject) -> None:
    """Generate the claims of a test contract from compiled `.wasm` and ABI files.

    Nothing is compiled here: the test contract's `output/` directory (`.wasm` and `.abi.json`) and
    the contracts listed in `kasmer.json` must already be built, by whatever means.
    """
    from kmultiversx import kasmer

    krun, test_endpoints, sym_conf, init_subst = KasmerSetup.load_from_project(project)
    output_dir = project.test_dir / 'generated_claims'

    logger.info(f'Generating claims: {output_dir}')
    kasmer.generate_claims(krun, test_endpoints, sym_conf, init_subst, output_dir)
