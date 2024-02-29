import { BrowserRouter, Route, Routes } from "react-router-dom";
import {
  Container,
  Content,
  Footer,
  Heading,
  Navbar,
  Section,
  Tile,
} from "react-bulma-components";

import "./App.css";
import { LoginStatus } from "./LoginStatus";
import { PostLoginRedirect } from "./PostLoginRedirect";
import { AibForm } from "./AibForm";

function App() {
  return (
    <BrowserRouter basename="/app">
      <PostLoginRedirect />
      <Navbar aria-label="main navigation" px="6">
        <Navbar.Brand>
          <Navbar.Item href="/app">
            <img alt="Dumpster fire" src="/app/logos/dumpster.svg" />
          </Navbar.Item>
        </Navbar.Brand>
        <Navbar.Container align="left">
          <Navbar.Item href="/app/" active>
            Home
          </Navbar.Item>
          <Navbar.Item href="https://github.com/travisbrown/archivindex-builder">
            About
          </Navbar.Item>
        </Navbar.Container>
        <Navbar.Container align="right">
          <LoginStatus />
        </Navbar.Container>
      </Navbar>
      <Section>
        <Tile kind="ancestor">
          <Tile kind="parent">
            <Tile kind="child" px="4" pb="4">
              <Container>
                <AibForm />
              </Container>
            </Tile>
          </Tile>
        </Tile>
        <Routes>
          <Route path="/" />
        </Routes>
      </Section>
      <Footer>
        <Container>
          <Content style={{ textAlign: "center" }}>
            <p>
              <a href="https://github.com/travisbrown/archivindex-builder">
                <strong>Archivindex Builder</strong>
              </a>{" "}
              is developed by{" "}
              <a href="https://twitter.com/travisbrown">Travis Brown</a>.
            </p>
          </Content>
        </Container>
      </Footer>
    </BrowserRouter>
  );
}

export default App;
