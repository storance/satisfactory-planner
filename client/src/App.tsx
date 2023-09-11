import { createSignal, onMount, For, Index } from 'solid-js'
import { parse_game_db, GameDatabase } from './game';
import solidLogo from './assets/solid.svg'
import viteLogo from '/vite.svg'
import './App.css'

function App() {
  const [count, setCount] = createSignal(0);
  const [gameDB, setGameDB] = createSignal(new GameDatabase(new Map(), new Map(), new Map(), new Map()));

  onMount(async () => {
    const res = await fetch(import.meta.env.VITE_API_URL + "api/1/database");
    setGameDB(parse_game_db(await res.json()));
  });

  return (
    <>
      <ul>
        <For each={Array.from(gameDB().recipes.values())}>{(item, i) =>
          <li>{item.name}</li>
        }</For>
      </ul>
      <div>
        <a href="https://vitejs.dev" target="_blank">
          <img src={viteLogo} class="logo" alt="Vite logo" />
        </a>
        <a href="https://solidjs.com" target="_blank">
          <img src={solidLogo} class="logo solid" alt="Solid logo" />
        </a>
      </div>
      <h1>Vite + Solid</h1>
      <div class="card">
        <button onClick={() => setCount((count) => count + 1)}>
          count is {count()}
        </button>
        <p>
          Edit <code>src/App.tsx</code> and save to test HMR
        </p>
      </div>
      <p class="read-the-docs">
        Click on the Vite and Solid logos to learn more
      </p>
    </>
  )
}

export default App
