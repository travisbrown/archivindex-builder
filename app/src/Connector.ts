import type { APIConnector } from "@elastic/search-ui";
import { RequestState, ResponseState, Filter, QueryConfig, AutocompleteQueryConfig, AutocompleteResponseState } from "@elastic/search-ui";

class Connector implements APIConnector {
    async onSearch(
        state: RequestState,
        queryConfig: QueryConfig
    ): Promise<ResponseState> {
        const { searchTerm, resultsPerPage, current, filters, sort } = state;
        const api_response: any = await fetch(`${process.env.REACT_APP_API_ROOT}/search`, {
            method: "POST",
            headers: {
                "Content-Type": "application/json"
            },
            body: JSON.stringify({
                searchTerm,
                resultsPerPage,
                current,
                filters,
                sort
            })
        }).then(response => response.json());

        const { patterns, years, pages } = api_response;

        console.log(api_response);

        let count = 0;

        const pattern_facets = Object.entries(patterns).map((entry) => {
            count += entry[1] as any;
            return ({
                value: entry[0],
                count: entry[1],
                selected: true
            });
        });

        const year_facets = Object.entries(years).map((entry) => {
            return ({
                value: entry[0],
                count: entry[1],
                selected: true
            });
        });

        year_facets.reverse();

        const facets = {
            pattern: [{
                data: pattern_facets,
                type: "value"
            }],
            year: [{
                data: year_facets,
                type: "value"
            }]
        };

        const results = pages.map((page: any) => {
            page.id = { raw: page.surt };
            page._meta = { id: page.surt };
            page.snapshots.forEach((snapshot: any) => {
                snapshot.raw = snapshot.snippet;
            });

            return page;
        });

        const response: ResponseState = {
            resultSearchTerm: searchTerm!,
            totalPages: count / resultsPerPage!,
            pagingStart:
                (current! - 1) * resultsPerPage! + 1,
            pagingEnd:
                current! * resultsPerPage!,
            wasSearched: true,
            totalResults: count,
            facets,
            results,
            requestId: "",
            rawResponse: null
        };
        return response;
    }

    async onAutocomplete(
        state: RequestState,
        queryConfig: AutocompleteQueryConfig
    ): Promise<AutocompleteResponseState> {
        return {
            autocompletedResults: [],
            autocompletedResultsRequestId: "",
            autocompletedSuggestions: {},
            autocompletedSuggestionsRequestId: ""
        };
    }

    onResultClick(params: any): void {
        console.log(
            "perform a call to the API to highlight a result has been clicked"
        );
    }

    onAutocompleteResultClick(params: any): void {
        console.log(
            "perform a call to the API to highlight an autocomplete result has been clicked"
        );
    }
}

export default Connector;