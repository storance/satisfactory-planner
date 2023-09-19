import { For } from "solid-js";
import { Container, Nav, Navbar, Tab, Tabs } from "solid-bootstrap";
import { AiFillFolderAdd } from "solid-icons/ai";
import { useFactories } from "./FactoriesContext";
import { FactoryState } from './FactoriesContext';
import { useGameDatabase } from "./GameDatabaseContext";
import Factory from "./Factory";

export default function App() {
    const [factoryState, actions] = useFactories();
    const [gameDB] = useGameDatabase();

    const updateActiveFactory = (eventKey: string | null) => {
        if (eventKey === "add") {
            actions.addFactory(FactoryState.default(gameDB));
        } else if (eventKey !== null) {
            actions.updateActiveFactoryId(eventKey);
        }
    };

    return (
        <>
            <Container fluid="xl">
                <Navbar expand="lg" class="bg-dark navbar-dark">
                    <Container>
                        <Navbar.Brand href="#home">Satisfactory Planner</Navbar.Brand>
                        <Navbar.Toggle aria-controls="basic-navbar-nav" />
                        <Navbar.Collapse id="basic-navbar-nav">
                            <Nav class="me-auto">
                                <Nav.Link href="#">Home</Nav.Link>
                            </Nav>
                        </Navbar.Collapse>
                    </Container>
                </Navbar>
                <Tabs
                    activeKey={factoryState.activeFactoryId}
                    onSelect={updateActiveFactory}
                >
                    <For each={factoryState.factories}>{(factory, index) => (
                        <Tab eventKey={factory.id} title={factory.name}>
                            <Factory factoryIndex={index()} factory={factory} />
                        </Tab>
                    )}</For>
                    <Tab eventKey={"add"} title={<AiFillFolderAdd />}></Tab>
                </Tabs>
            </Container>
        </>
    )
}
