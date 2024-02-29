import { SearchProvider, SearchBox } from "@elastic/react-search-ui";
import { SearchResult } from "@elastic/search-ui";
import Connector from "./Connector";
import { Facet, Results, ResultsPerPage, Paging, PagingInfo, ErrorBoundary, WithSearch } from "@elastic/react-search-ui";
import { Layout } from "@elastic/react-search-ui-views";
import React from "react";
import { Button, Form } from "react-bulma-components";
import "@elastic/react-search-ui-views/lib/styles/styles.css";
import "./overrides.scss";

const CustomResultView = ({
  result,
  onClickLink
}: {
  result: SearchResult;
  onClickLink: () => void;
}) => {

  return (

    < li className="sui-result" >
      <div className="result-header">
        <h3>
          {result.title}
        </h3>
        <div><h4>
          {result.url}
        </h4></div>
      </div>
      <div className="sui-result__body">
        <ol>
          {result.snapshots.map((snapshot: any) => {
            const date = new Date(Date.parse(snapshot.timestamp)) as any;
            const dateString = date.toLocaleDateString('en-US', { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric', hour: 'numeric', minute: 'numeric', second: 'numeric', hour12: true });

            return <li>
              <div>
                <h4><a onClick={onClickLink} href={snapshot.url}>{dateString}</a></h4>
                <p dangerouslySetInnerHTML={{ __html: snapshot.snippet }} />
              </div>
            </li>;
          })}
        </ol>
      </div>
    </li >
  );
};

export function AibForm() {
  const connector = new Connector();

  const facets = {};

  const configurationOptions = {
    debug: true,
    apiConnector: connector,
    initialState: { searchTerm: "test", resultsPerPage: 20 },
    filters: [],
    searchQuery: {
      search_fields: {
        content: {},
        pattern: {
        }
      },
      result_fields: {
        content: {
          snippet: {}
        }
      },
      disjunctiveFacets: ["pattern", "year"],
      facets
    },
    alwaysSearchOnInitialLoad: true
  };

  const inputProps = {
    className: "is-light"
  };

  return <SearchProvider config={configurationOptions}>
    < WithSearch
      mapContextToProps={({ wasSearched }) => ({
        wasSearched
      })
      }
    >
      {({ wasSearched }) => {
        return (<div className="App" >
          <ErrorBoundary>
            <Layout
              header={
                <SearchBox autocompleteSuggestions={true}
                  inputView={({ getAutocomplete, getInputProps, getButtonProps }) => (
                    <>
                      <div className="sui-search-box__wrapper">
                        <input
                          {...getInputProps({
                          })}
                        />
                        {getAutocomplete()}
                      </div>
                      <div className="control">
                        <button {...getButtonProps({ className: "is-info button important_style" })}>Search</button>
                      </div>
                    </>
                  )} />
              }
              sideContent={
                <div>
                  <Facet key={"1"} field={"pattern"} label={"Collection"} show={100} filterType="any" />
                  <Facet key={"2"} field={"year"} label={"Year"} show={10} filterType="any" />
                </div>
              }
              bodyContent={<Results shouldTrackClickThrough={true} resultView={CustomResultView} />}
              bodyHeader={
                <React.Fragment>
                  {wasSearched && <PagingInfo />}
                  {wasSearched && <ResultsPerPage />}
                </React.Fragment>
              }
              bodyFooter={<Paging />}
            /></ErrorBoundary >
        </div>
        );
      }}
    </WithSearch >
  </SearchProvider >;
}
