// utility functions for formatting data


// Convert a date of `yyyymmdd` to a date object
export function dateFromYYYYMMDD(yyyymmdd) {
    return new Date(
        yyyymmdd.replace(/(\d{4})(\d{2})(\d{2})/, "$1-$2-$3")
    );
}
