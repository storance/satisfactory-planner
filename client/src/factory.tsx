import { FactoryState } from './FactoriesContext';

export default function Factory(props: { factory: FactoryState, factoryIndex: number }) {
    return (
        <>
            #{props.factoryIndex}: {props.factory.name}
        </>
    );
}