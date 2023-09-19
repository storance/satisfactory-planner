import { createContext, useContext, ParentComponent } from "solid-js";
import { createStore } from "solid-js/store";
import { GameDatabase } from "./game";
import { v4 as uuidv4 } from "uuid";
import { useGameDatabase } from "./GameDatabaseContext";

export enum FactoryOutputType {
    PerMinute = "per_minute",
    Maximize = "maximize"
}

export class FactoryOutput {
    item: string | null;
    type: FactoryOutputType;
    amount: number;

    constructor(item: string | null, type: FactoryOutputType, amount: number) {
        this.item = item;
        this.type = type;
        this.amount = amount;
    }
}

export class FactoryInput {
    item: string | null;
    amount: number

    constructor(item: string | null, amount: number) {
        this.item = item;
        this.amount = amount;
    }
}

export class FactoryState {
    id: string;
    name: string;
    inputs: FactoryInput[];
    resourceLimits: Map<string, number>;
    outputs: FactoryOutput[];
    enabledRecipes: string[];

    constructor(name: string,
        inputs: FactoryInput[],
        resourceLimits: Map<string, number>,
        outputs: FactoryOutput[],
        enabledRecipes: string[]) {

        this.id = uuidv4();
        this.name = name;
        this.inputs = inputs;
        this.resourceLimits = resourceLimits;
        this.outputs = outputs;
        this.enabledRecipes = enabledRecipes;
    }

    static default(gameDB: GameDatabase): FactoryState {
        const resourceLimits = new Map<string, number>(gameDB.resourceLimits);
        const enabledRecipes = Array.from(gameDB.recipes.values()).filter(r => !r.alternate).map(r => r.key);
        const outputs = [new FactoryOutput(null, FactoryOutputType.PerMinute, 10)];

        return new FactoryState('Untitled Factory', [], resourceLimits, outputs, enabledRecipes);
    }
}

export class FactoriesState {
    factories: FactoryState[];
    activeFactoryId: string;

    constructor(factories: FactoryState[], activeFactoryId: string) {
        this.factories = factories;
        this.activeFactoryId = activeFactoryId;
    }

    static empty(): FactoriesState {
        return new FactoriesState([], "");
    }
}

export type FactoriesContextValue = [
    state: FactoriesState,
    actions: {
        addInput: (factoryIndex: number, input: FactoryInput) => void;
        updateInputItem: (factoryIndex: number, input: number, item: string) => void;
        updateInputAmount: (factoryIndex: number, input: number, amount: number) => void;
        removeInput: (factoryIndex: number, input: number) => void

        addOutput: (factoryIndex: number, output: FactoryOutput) => void;
        updateOutputItem: (factoryIndex: number, output: number, item: string) => void;
        updateOutputType: (factoryIndex: number, output: number, type: FactoryOutputType) => void;
        updateOutputAmount: (factoryIndex: number, output: number, amount: number) => void;
        removeOutput: (factoryIndex: number, output: number) => void;

        updateResourceLimit: (factoryIndex: number, item: string, amount: number) => void;
        setResourceLimits: (factoryIndex: number, resourceLimits: Map<string, number>) => void;

        addRecipe: (factoryIndex: number, recipe: string) => void;
        addAllRecipes: (factoryIndex: number, recipes: string[]) => void;
        removeRecipe: (factoryIndex: number, recipe: string) => void;
        removeAllRecipes: (factoryIndex: number, recipes: string[]) => void;

        addFactory: (factory: FactoryState) => void;
        cloneFactory: (factoryIndex: number) => void;
        removeFactory: (factoryIndex: number) => void;
        updateActiveFactoryId: (factoryId: string) => void;
    }
];

const FactoriesContext = createContext<FactoriesContextValue>([
    FactoriesState.empty(),
    {
        addInput: () => undefined,
        updateInputItem: () => undefined,
        updateInputAmount: () => undefined,
        removeInput: () => undefined,
        addOutput: () => undefined,
        updateOutputItem: () => undefined,
        updateOutputType: () => undefined,
        updateOutputAmount: () => undefined,
        removeOutput: () => undefined,
        updateResourceLimit: () => undefined,
        setResourceLimits: () => undefined,
        addRecipe: () => undefined,
        addAllRecipes: () => undefined,
        removeRecipe: () => undefined,
        removeAllRecipes: () => undefined,
        addFactory: () => undefined,
        cloneFactory: () => undefined,
        removeFactory: () => undefined,
        updateActiveFactoryId: () => undefined
    },
]);

function removeIndex<T>(array: readonly T[], index: number): T[] {
    return [...array.slice(0, index), ...array.slice(index + 1)];
}

export const FactoriesProvider: ParentComponent<{}> = (props) => {
    const [gameDB, _] = useGameDatabase();
    const defaultFactory = FactoryState.default(gameDB);
    const [state, setState] = createStore<FactoriesState>(new FactoriesState([defaultFactory], defaultFactory.id));

    const addInput = (factoryIndex: number, input: FactoryInput) => {
        setState('factories', factoryIndex, 'inputs', i => [...i, input]);
    };
    const updateInputItem = (factoryIndex: number, input: number, item: string) => {
        setState('factories', factoryIndex, 'inputs', input, 'item', item);
    };
    const updateInputAmount = (factoryIndex: number, input: number, amount: number) => {
        setState('factories', factoryIndex, 'inputs', input, 'amount', amount);
    };
    const removeInput = (factoryIndex: number, input: number) => {
        setState('factories', factoryIndex, 'inputs', i => removeIndex(i, input));
    };

    const addOutput = (factoryIndex: number, output: FactoryOutput) => {
        setState('factories', factoryIndex, 'outputs', o => [...o, output]);
    };
    const updateOutputItem = (factoryIndex: number, output: number, item: string) => {
        setState('factories', factoryIndex, 'outputs', output, 'item', item);
    };
    const updateOutputType = (factoryIndex: number, output: number, type: FactoryOutputType) => {
        setState('factories', factoryIndex, 'outputs', output, 'type', type);
    };
    const updateOutputAmount = (factoryIndex: number, output: number, amount: number) => {
        setState('factories', factoryIndex, 'outputs', output, 'amount', amount);
    };
    const removeOutput = (factoryIndex: number, output: number) => {
        setState('factories', factoryIndex, 'outputs', o => removeIndex(o, output));
    };

    const updateResourceLimit = (factoryIndex: number, item: string, amount: number) => {
        setState('factories', factoryIndex, 'resourceLimits', m => {
            let newResourceLimits = new Map(m);
            newResourceLimits.set(item, amount);
            return newResourceLimits;
        });
    };
    const setResourceLimits = (factoryIndex: number, resourceLimits: Map<string, number>) => {
        setState('factories', factoryIndex, 'resourceLimits', resourceLimits);
    }

    const addRecipe = (factoryIndex: number, recipe: string) => {
        setState('factories', factoryIndex, 'enabledRecipes', er => [...er, recipe]);
    }
    const addAllRecipes = (factoryIndex: number, recipes: string[]) => {
        setState('factories', factoryIndex, 'enabledRecipes', er => [...er, ...recipes]);
    }
    const removeRecipe = (factoryIndex: number, recipe: string) => {
        setState('factories', factoryIndex, 'enabledRecipes', er => er.filter(r => r != recipe));
    }
    const removeAllRecipes = (factoryIndex: number, recipes: string[]) => {
        setState('factories', factoryIndex, 'enabledRecipes', er => er.filter(r => !recipes.includes(r)));
    }

    const addFactory = (factory: FactoryState) => setState('factories', f => [...f, factory]);
    const cloneFactory = (factoryIndex: number) => setState('factories', f => [...f, f[factoryIndex]]);
    const removeFactory = (factoryIndex: number) => setState('factories', f => removeIndex(f, factoryIndex));
    const updateActiveFactoryId = (factoryId: string) => setState('activeFactoryId', factoryId);

    const contextValue: FactoriesContextValue = [state, {
        addInput,
        updateInputItem,
        updateInputAmount,
        removeInput,
        addOutput,
        updateOutputItem,
        updateOutputType,
        updateOutputAmount,
        removeOutput,
        updateResourceLimit,
        setResourceLimits,
        addRecipe,
        addAllRecipes,
        removeRecipe,
        removeAllRecipes,
        addFactory,
        cloneFactory,
        removeFactory,
        updateActiveFactoryId,
    }];

    return (
        <FactoriesContext.Provider value={contextValue}>
            {props.children}
        </FactoriesContext.Provider>
    )
}

export const useFactories = () => useContext(FactoriesContext);