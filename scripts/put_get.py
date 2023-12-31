import asyncio
import aiohttp
import sys
import random

from common import (
    NUM_HOST_BENCHMARK_PEER,
    HOSTS,
    SERVICE as PLAZA,
    INNER_K,
    INNER_N,
    OUTER_K,
    OUTER_N,
    PROTOCOL,
    NUM_OPERATION,
    NUM_TOTAL_PEER,
)


ARGV = dict(enumerate(sys.argv))
NUM_CONCURRENT = int(ARGV.get(1, "1"))
assert NUM_CONCURRENT <= NUM_OPERATION


def to_timestamp(system_time):
    return (
        system_time["secs_since_epoch"]
        + system_time["nanos_since_epoch"] / 1000 / 1000 / 1000
    )


async def ready():
    async with aiohttp.ClientSession() as session:
        ready = False
        while not ready:
            async with session.get(f"{PLAZA}/ready") as resp:
                ready = await resp.json()


async def put_get(i, peer):
    async with aiohttp.ClientSession() as session:
        print(f"{i:02} commit put operation on {peer}")
        async with session.post(f"{peer}/benchmark/{PROTOCOL}") as resp:
            benchmark_id = await resp.json()
        while True:
            await asyncio.sleep(1)
            async with session.get(f"{peer}/benchmark/{benchmark_id}") as resp:
                result = await resp.json()
                if result["put_end"]:
                    break
        latency = to_timestamp(result["put_end"]) - to_timestamp(result["put_start"])
        print(
            f",{peer},put,{latency},{PROTOCOL},{INNER_K},{INNER_N},{OUTER_K},{OUTER_N},{NUM_CONCURRENT},{NUM_TOTAL_PEER}"
        )
        await asyncio.sleep(1)

        print(f"{i:02} commit get operation on {peer}")
        await session.post(f"{peer}/benchmark/{benchmark_id}/{PROTOCOL}/get")
        while True:
            await asyncio.sleep(1)
            async with session.get(f"{peer}/benchmark/{benchmark_id}") as resp:
                result = await resp.json()
                if result["get_end"]:
                    break
        latency = to_timestamp(result["get_end"]) - to_timestamp(result["get_start"])
        print(
            f",{peer},get,{latency},{PROTOCOL},{INNER_K},{INNER_N},{OUTER_K},{OUTER_N},{NUM_CONCURRENT},{NUM_TOTAL_PEER}"
        )
        await asyncio.sleep(1)
    return peer


def choose_peer(peers, working_peers):
    assert len(working_peers) != len(peers)
    while True:
        peer = random.choice(peers)
        if peer not in working_peers:
            working_peers.add(peer)
            return peer


async def main():
    print(
        "comment,peer,operation,latency,protocol,inner_k,inner_n,outer_k,outer_n,num_concurrent,num_participant"
    )
    await ready()
    peers = [
        f"http://{host}:{10000 + index}"
        for host in HOSTS
        for index in range(NUM_HOST_BENCHMARK_PEER)
    ]
    tasks = []
    working_peers = set()
    for i in range(NUM_CONCURRENT):
        # await asyncio.sleep(5)
        tasks.append(asyncio.create_task(put_get(i, choose_peer(peers, working_peers))))
    num_operation = NUM_CONCURRENT
    while tasks:
        done_tasks, tasks = await asyncio.wait(
            tasks, return_when=asyncio.FIRST_COMPLETED
        )
        for done_task in done_tasks:
            if done_task.exception():
                print(done_task.exception())
            elif num_operation < NUM_OPERATION:
                peer = done_task.result()
                working_peers.remove(peer)
                tasks.add(
                    asyncio.create_task(
                        put_get(num_operation, choose_peer(peers, working_peers))
                    )
                )
                num_operation += 1


if __name__ == "__main__":
    asyncio.run(main())
