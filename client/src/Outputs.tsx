import { createSignal, For } from 'solid-js';
import { Select } from "@thisbeyond/solid-select";
import { Item } from './game';


function Outputs(props: any) {
    const [outputs, setOutputs] = createSignal([]);

    const format = (item: Item, type: string) => (type === "option" ? item.name : item.key);
    return (
        <>
            <For each={outputs()}>{(output, i) =>
                <Select options={props.game_db().items} format={format} onChange={update} />
            }</For>
        </>
    );
}

export default Outputs;