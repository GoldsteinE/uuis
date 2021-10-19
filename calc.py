from __future__ import annotations

import sys
import json
import asyncio
from dataclasses import dataclass

from typing import Any, Dict, List, Tuple


class NoInput:
    def __str__(self) -> str:
        return '/no input/'

    def __bool__(self) -> bool:
        return False

    def strip(self) -> NothingYet:
        return self


class NothingYet:
    def __str__(self) -> str:
        return '/nothing yet/'


@dataclass
class Previous:
    inp: str
    out: object
    broken: bool = False

    @classmethod
    def empty(cls) -> Previous:
        return cls(NoInput(), NothingYet())

    @property
    def equals_sign(self) -> str:
        return '≠' if self.broken else '='


class Calculator:
    def __init__(self, r: asyncio.StreamReader, w: asyncio.StreamWriter) -> None:
        self.r = r
        self.w = w
        self.previous: List[Previous] = [Previous.empty()]

    def iter_previous(self, *, include_last: bool = True):
        last = len(self.previous) - 1
        for idx, item in enumerate(reversed(self.previous)):
            if idx == 0 and not include_last:
                continue

            yield (last - idx, item)

    @classmethod
    async def connect(cls, host: str, port: int) -> Calculator:
        r, w = await asyncio.open_connection(host, port)
        return cls(r, w)

    async def send(self, message: Any) -> None:
        self.w.write(json.dumps(message).encode())
        self.w.write(b'\n')
        await self.w.drain()

    async def recv(self) -> Any:
        return json.loads(await self.r.readline())

    async def register(self) -> int:
        await self.send({'subscribe_to': 0b1101, 'protocol_version': 0, 'matcher': 'none'})

        first_message = await self.recv()
        if first_message['key'] == 'busy':
            return (await self.recv())['data']
        else:
            return first_message['data']

    async def set_choices(self) -> None:
        await self.send(
            {
                'key': 'set_choices',
                'data': {
                    'options': [
                        {
                            'text': f'{idx}: {p.inp.strip()} {p.equals_sign} {p.out}',
                            'id': idx,
                            'priority': -idx,
                        }
                        for idx, p in self.iter_previous()
                    ],
                }
            }
        )

    async def clear_input(self) -> None:
        await self.send({'key': 'set_input', 'data': ''})

    def generate_locals(self) -> Dict[str, object]:
        return {
            f'_{idx}': p.out
            for idx, p
            in self.iter_previous(include_last=False)
        }

    async def run(self) -> None:
        client_id = await self.register()

        while True:
            await self.set_choices()
            message = await self.recv()
            key = message['key']
            data = message.get('data')

            if key == 'select':
                if data is None:
                    if not isinstance(self.previous[-1].out, NothingYet):
                        self.previous.append(Previous.empty()) 
                        await self.clear_input()
                else:
                    print(self.previous[data].out)
                    break

            if key == 'input_change':
                curr = self.previous[-1]
                try:
                    curr.broken = False
                    out = eval(data, {}, self.generate_locals())
                except Exception:
                    if data:
                        curr.inp = data
                        if not isinstance(curr.out, NothingYet):
                            curr.broken = True
                    else:
                        curr.inp = NoInput()
                else:
                    self.previous[-1] = Previous(data, out)

            if key == 'window_closed':
                return

        await self.send(json.dumps({'key': 'stop'}))


async def main() -> None:
    calc = await Calculator.connect('127.0.0.1', 5555)
    await calc.run()


if __name__ == '__main__':
    asyncio.run(main())
