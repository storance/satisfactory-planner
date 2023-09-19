import { ParentComponent, createContext, onMount, useContext } from "solid-js";
import { createStore } from "solid-js/store";
import { GameDatabase } from "./game";

export type GameDatabaseContextValue = [
    state: GameDatabase,
    actions: {
        setState: (gameDB: GameDatabase) => void;
    }
];

const GameDatabaseContext = createContext<GameDatabaseContextValue>([
    new GameDatabase(new Map(), new Map(), new Map(), new Map()),
    { setState: () => undefined },
]);

export const GameDatabaseProvider: ParentComponent<{}> = (props) => {
    const [state, setState] = createStore<GameDatabase>(new GameDatabase(new Map(), new Map(), new Map(), new Map()));

    onMount(async () => {
        const res = await fetch(import.meta.env.VITE_API_URL + "api/1/database");
        setState(GameDatabase.fromJson(await res.json()));
    });

    const contextValue: GameDatabaseContextValue = [state, { setState }];
    return (
        <GameDatabaseContext.Provider value={contextValue}>
            {props.children}
        </GameDatabaseContext.Provider>
    );
}

export const useGameDatabase = () => useContext(GameDatabaseContext);