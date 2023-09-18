import { createContext, useContext, ParentComponent } from "solid-js";
import { createStore } from "solid-js/store";
import { ItemPerMinute } from "./game";

export enum FactoryOutputType {
    PerMinute = "per_minute",
    Maximize = "maximize"
}

export class FactoryOutput {
    item: string;
    type: FactoryOutputType;
    amount: number;

    constructor(item: string, type: FactoryOutputType, amount: number) {
        this.item = item;
        this.type = type;
        this.amount = amount;
    }
}

export class Factory {
    name: string;
    inputs: ItemPerMinute[];
    resourceLimits: Map<string, number>;
    outputs: FactoryOutput[];
    enabled_recipes: string[];

    constructor(name: string,
        inputs: ItemPerMinute[],
        resourceLimits: Map<string, number>,
        outputs: FactoryOutput[],
        enabled_recipes: string[]) {

        this.name = name;
        this.inputs = inputs;
        this.resourceLimits = resourceLimits;
        this.outputs = outputs;
        this.enabled_recipes = enabled_recipes;
    }

    static empty(name: string): Factory {
        return new Factory(name, [], new Map<string, number>(), [], []);
    }
}

export type FactoriesContextValue = [
    state: Factory[],
    actions: {
        addInput: (factory: number, input: ItemPerMinute) => void;
        updateInputItem: (factory: number, input: number, item: string) => void;
        updateInputAmount: (factory: number, input: number, amount: number) => void;
        removeInput: (factory: number, input: number) => void

        addOutput: (factory: number, output: FactoryOutput) => void;
        updateOutputItem: (factory: number, output: number, item: string) => void;
        updateOutputType: (factory: number, output: number, type: FactoryOutputType) => void;
        updateOutputAmount: (factory: number, output: number, amount: number) => void;
        removeOutput: (factory: number, output: number) => void;

        updateResourceLimit: (factory: number, item: string, amount: number) => void;
        setResourceLimits: (factory: number, resourceLimits: Map<string, number>) => void;

        addRecipe: (factory: number, recipe: string) => void;
        addAllRecipes: (factory: number, recipes: string[]) => void;
        removeRecipe: (factory: number, recipe: string) => void;
        removeAllRecipes: (factory: number, recipes: string[]) => void;

        addFactory: (factory: Factory) => void;
        cloneFactory: (factory: number) => void;
        removeFactory: (factory: number) => void;

    }
];

const FactoriesContext = createContext<FactoriesContextValue>([
    [],
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
    },
]);

function removeIndex<T>(array: readonly T[], index: number): T[] {
    return [...array.slice(0, index), ...array.slice(index + 1)];
}

export const FactoriesProvider: ParentComponent<{}> = (props) => {
    const [state, setState] = createStore<Factory[]>([]);

    const addInput = (factory: number, input: ItemPerMinute) => setState(factory, 'inputs', i => [...i, input]);
    const updateInputItem = (factory: number, input: number, item: string) => setState(factory, 'inputs', input, 'item', item);
    const updateInputAmount = (factory: number, input: number, amount: number) => setState(factory, 'inputs', input, 'amount', amount);
    const removeInput = (factory: number, input: number) => setState(factory, 'inputs', i => removeIndex(i, input));

    const addOutput = (factory: number, output: FactoryOutput) => setState(factory, 'outputs', o => [...o, output]);
    const updateOutputItem = (factory: number, output: number, item: string) => setState(factory, 'outputs', output, 'item', item);
    const updateOutputType = (factory: number, output: number, type: FactoryOutputType) => setState(factory, 'outputs', output, 'type', type);
    const updateOutputAmount = (factory: number, output: number, amount: number) => setState(factory, 'outputs', output, 'amount', amount);
    const removeOutput = (factory: number, output: number) => setState(factory, 'outputs', o => removeIndex(o, output));

    const updateResourceLimit = (factory: number, item: string, amount: number) => setState(factory, 'resourceLimits', m => {
        let newResourceLimits = new Map(m);
        newResourceLimits.set(item, amount);
        return newResourceLimits;
    });
    const setResourceLimits = (factory: number, resourceLimits: Map<string, number>) => setState(factory, 'resourceLimits', resourceLimits);

    const addRecipe = (factory: number, recipe: string) => setState(factory, 'enabled_recipes', er => [...er, recipe]);
    const addAllRecipes = (factory: number, recipes: string[]) => setState(factory, 'enabled_recipes', er => [...er, ...recipes]);
    const removeRecipe = (factory: number, recipe: string) => setState(factory, 'enabled_recipes', er => er.filter(r => r != recipe));
    const removeAllRecipes = (factory: number, recipes: string[]) => setState(factory, 'enabled_recipes', er => er.filter(r => !recipes.includes(r)));

    const addFactory = (factory: Factory) => setState(f => [...f, factory]);
    const cloneFactory = (factory: number) => setState(f => [...f, f[factory]]);
    const removeFactory = (factory: number) => setState(f => removeIndex(f, factory));

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
        removeFactory
    }];

    return (
        <FactoriesContext.Provider value={contextValue}>
            {props.children}
        </FactoriesContext.Provider>
    )
}

export const useFactories = () => useContext(FactoriesContext);