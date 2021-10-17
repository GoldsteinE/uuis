import sys
import json
import asyncio


async def main():
    options = list(map(str.strip, sys.stdin))
    r, w = await asyncio.open_connection('127.0.0.1', 5555)
    w.write((json.dumps({'subscribe_to': 1, 'protocol_version': 0}) + '\n').encode())
    await w.drain()
    client_id = json.loads(await r.readline())['Registered']
    print(f'Client ID: {client_id}', file=sys.stderr)

    w.write(
        (
            json.dumps(
                {
                    'SetChoices': {
                        'options': [
                            {'text': option, 'id': idx}
                            for idx, option
                            in enumerate(options)
                        ],
                    }
                }
            )
            + '\n'
        ).encode()
    )
    await w.drain()

    option = json.loads(await r.readline())['Select']
    print(options[option])


if __name__ == '__main__':
    asyncio.run(main())
