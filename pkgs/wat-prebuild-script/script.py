import asyncio
import os
import sys

from enum import Enum
from subprocess import DEVNULL


FLAKE = os.environ.get('FLAKE')
TARGETS = os.environ.get('TARGETS').split(':')

Result = Enum('Result', ['SUCCESS', 'BUILD_FAIL', 'BUILD_TIMEOUT', 'EVAL_FAIL', 'EVAL_TIMEOUT'], start=0)

async def buildTarget(target):
    process = await asyncio.create_subprocess_exec(
        'nix',
        'path-info',
        '--derivation',
        f'{FLAKE}#{target}',
        stdout=DEVNULL, stderr=DEVNULL)
    try:
        eval_result = await asyncio.wait_for(process.wait(), 300)
    except asyncio.exceptions.TimeoutError:
        return (target, Result.EVAL_TIMEOUT)
    if eval_result != 0:
        return (target, Result.EVAL_FAIL)
    process = await asyncio.create_subprocess_exec(
        'nix',
        'build',
        '--no-link',
        f'{FLAKE}#{target}',
        stdout=DEVNULL, stderr=DEVNULL)
    try:
        build_result = await asyncio.wait_for(process.wait(), 7200)
    except asyncio.exceptions.TimeoutError:
        return (target, Result,BUILD_TIMEOUT)
    if build_result != 0:
        return (target, Result.BUILD_FAIL)
    return (target, Result.SUCCESS)


async def main():
    jobs = [buildTarget(target) for target in TARGETS]
    results = await asyncio.gather(*jobs)
    for (target, result) in results:
        print(result.value, result.name, target)
    sys.exit(max(map(lambda x: x[1].value, results)))


if __name__ == '__main__':
    asyncio.run(main())
