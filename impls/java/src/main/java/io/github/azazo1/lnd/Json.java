package io.github.azazo1.lnd;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

final class Json {
    private Json() {
    }

    static Object parse(String text) throws LndException {
        Parser parser = new Parser(text);
        Object value = parser.parseValue();
        parser.skipWhitespace();
        if (!parser.isAtEnd()) {
            throw new LndException("invalid json: trailing content");
        }
        return value;
    }

    static String stringify(Object value) throws LndException {
        StringBuilder builder = new StringBuilder();
        writeValue(builder, value);
        return builder.toString();
    }

    private static void writeValue(StringBuilder builder, Object value) throws LndException {
        if (value == null) {
            builder.append("null");
            return;
        }
        if (value instanceof String) {
            writeString(builder, (String) value);
            return;
        }
        if (value instanceof Number || value instanceof Boolean) {
            builder.append(value.toString());
            return;
        }
        if (value instanceof Map) {
            builder.append('{');
            boolean first = true;
            for (Map.Entry<?, ?> entry : ((Map<?, ?>) value).entrySet()) {
                if (!first) {
                    builder.append(',');
                }
                first = false;
                writeString(builder, String.valueOf(entry.getKey()));
                builder.append(':');
                writeValue(builder, entry.getValue());
            }
            builder.append('}');
            return;
        }
        if (value instanceof List) {
            builder.append('[');
            boolean first = true;
            for (Object item : (List<?>) value) {
                if (!first) {
                    builder.append(',');
                }
                first = false;
                writeValue(builder, item);
            }
            builder.append(']');
            return;
        }
        throw new LndException("unsupported json value: " + value.getClass().getName());
    }

    private static void writeString(StringBuilder builder, String value) {
        builder.append('"');
        for (int index = 0; index < value.length(); index++) {
            char ch = value.charAt(index);
            switch (ch) {
                case '"':
                    builder.append("\\\"");
                    break;
                case '\\':
                    builder.append("\\\\");
                    break;
                case '\b':
                    builder.append("\\b");
                    break;
                case '\f':
                    builder.append("\\f");
                    break;
                case '\n':
                    builder.append("\\n");
                    break;
                case '\r':
                    builder.append("\\r");
                    break;
                case '\t':
                    builder.append("\\t");
                    break;
                default:
                    if (ch < 0x20) {
                        builder.append(String.format("\\u%04x", (int) ch));
                    } else {
                        builder.append(ch);
                    }
            }
        }
        builder.append('"');
    }

    static Map<String, Object> asObject(Object value, String context) throws LndException {
        if (!(value instanceof Map)) {
            throw new LndException("expected json object for " + context);
        }
        Map<?, ?> raw = (Map<?, ?>) value;
        Map<String, Object> mapped = new LinkedHashMap<String, Object>();
        for (Map.Entry<?, ?> entry : raw.entrySet()) {
            mapped.put(String.valueOf(entry.getKey()), entry.getValue());
        }
        return mapped;
    }

    static List<Object> asArray(Object value, String context) throws LndException {
        if (!(value instanceof List)) {
            throw new LndException("expected json array for " + context);
        }
        List<?> raw = (List<?>) value;
        return new ArrayList<Object>(raw);
    }

    static String optString(Map<String, Object> object, String key) throws LndException {
        Object value = object.get(key);
        if (value == null) {
            return null;
        }
        if (!(value instanceof String)) {
            throw new LndException("expected string for field: " + key);
        }
        return (String) value;
    }

    static String requireString(Map<String, Object> object, String key) throws LndException {
        String value = optString(object, key);
        if (value == null) {
            throw new LndException("missing required string field: " + key);
        }
        return value;
    }

    static Long optLong(Map<String, Object> object, String key) throws LndException {
        Object value = object.get(key);
        if (value == null) {
            return null;
        }
        if (value instanceof Number) {
            return ((Number) value).longValue();
        }
        throw new LndException("expected number for field: " + key);
    }

    static long requireLong(Map<String, Object> object, String key) throws LndException {
        Long value = optLong(object, key);
        if (value == null) {
            throw new LndException("missing required number field: " + key);
        }
        return value.longValue();
    }

    static boolean optBoolean(Map<String, Object> object, String key, boolean defaultValue) throws LndException {
        Object value = object.get(key);
        if (value == null) {
            return defaultValue;
        }
        if (value instanceof Boolean) {
            return ((Boolean) value).booleanValue();
        }
        throw new LndException("expected boolean for field: " + key);
    }

    static List<String> optStringList(Map<String, Object> object, String key) throws LndException {
        Object value = object.get(key);
        if (value == null) {
            return new ArrayList<String>();
        }
        List<Object> raw = asArray(value, key);
        List<String> values = new ArrayList<String>(raw.size());
        for (Object item : raw) {
            if (!(item instanceof String)) {
                throw new LndException("expected string array item for field: " + key);
            }
            values.add((String) item);
        }
        return values;
    }

    static Map<String, String> optStringMap(Map<String, Object> object, String key) throws LndException {
        Object value = object.get(key);
        if (value == null) {
            return new LinkedHashMap<String, String>();
        }
        Map<String, Object> raw = asObject(value, key);
        Map<String, String> values = new LinkedHashMap<String, String>();
        for (Map.Entry<String, Object> entry : raw.entrySet()) {
            if (!(entry.getValue() instanceof String)) {
                throw new LndException("expected string map value for field: " + key);
            }
            values.put(entry.getKey(), (String) entry.getValue());
        }
        return values;
    }

    private static final class Parser {
        private final String text;
        private int index;

        private Parser(String text) {
            this.text = text;
            this.index = 0;
        }

        private boolean isAtEnd() {
            return index >= text.length();
        }

        private void skipWhitespace() {
            while (!isAtEnd()) {
                char ch = text.charAt(index);
                if (ch == ' ' || ch == '\n' || ch == '\r' || ch == '\t') {
                    index++;
                } else {
                    break;
                }
            }
        }

        private Object parseValue() throws LndException {
            skipWhitespace();
            if (isAtEnd()) {
                throw new LndException("invalid json: unexpected end of input");
            }
            char ch = text.charAt(index);
            switch (ch) {
                case '{':
                    return parseObject();
                case '[':
                    return parseArray();
                case '"':
                    return parseString();
                case 't':
                    parseLiteral("true");
                    return Boolean.TRUE;
                case 'f':
                    parseLiteral("false");
                    return Boolean.FALSE;
                case 'n':
                    parseLiteral("null");
                    return null;
                default:
                    if (ch == '-' || isDigit(ch)) {
                        return parseNumber();
                    }
                    throw new LndException("invalid json value starting at index " + index);
            }
        }

        private Map<String, Object> parseObject() throws LndException {
            expect('{');
            skipWhitespace();
            Map<String, Object> object = new LinkedHashMap<String, Object>();
            if (peek('}')) {
                index++;
                return object;
            }
            while (true) {
                skipWhitespace();
                String key = parseString();
                skipWhitespace();
                expect(':');
                Object value = parseValue();
                object.put(key, value);
                skipWhitespace();
                if (peek('}')) {
                    index++;
                    return object;
                }
                expect(',');
            }
        }

        private List<Object> parseArray() throws LndException {
            expect('[');
            skipWhitespace();
            List<Object> array = new ArrayList<Object>();
            if (peek(']')) {
                index++;
                return array;
            }
            while (true) {
                array.add(parseValue());
                skipWhitespace();
                if (peek(']')) {
                    index++;
                    return array;
                }
                expect(',');
            }
        }

        private String parseString() throws LndException {
            expect('"');
            StringBuilder builder = new StringBuilder();
            while (!isAtEnd()) {
                char ch = text.charAt(index++);
                if (ch == '"') {
                    return builder.toString();
                }
                if (ch == '\\') {
                    if (isAtEnd()) {
                        throw new LndException("invalid json string escape");
                    }
                    char escaped = text.charAt(index++);
                    switch (escaped) {
                        case '"':
                            builder.append('"');
                            break;
                        case '\\':
                            builder.append('\\');
                            break;
                        case '/':
                            builder.append('/');
                            break;
                        case 'b':
                            builder.append('\b');
                            break;
                        case 'f':
                            builder.append('\f');
                            break;
                        case 'n':
                            builder.append('\n');
                            break;
                        case 'r':
                            builder.append('\r');
                            break;
                        case 't':
                            builder.append('\t');
                            break;
                        case 'u':
                            builder.append(parseUnicodeEscape());
                            break;
                        default:
                            throw new LndException("invalid json escape: \\" + escaped);
                    }
                    continue;
                }
                builder.append(ch);
            }
            throw new LndException("unterminated json string");
        }

        private char parseUnicodeEscape() throws LndException {
            if (index + 4 > text.length()) {
                throw new LndException("invalid unicode escape");
            }
            int codePoint = 0;
            for (int offset = 0; offset < 4; offset++) {
                char ch = text.charAt(index++);
                int digit = Character.digit(ch, 16);
                if (digit < 0) {
                    throw new LndException("invalid unicode escape digit: " + ch);
                }
                codePoint = (codePoint << 4) | digit;
            }
            return (char) codePoint;
        }

        private Number parseNumber() throws LndException {
            int start = index;
            if (peek('-')) {
                index++;
            }
            consumeDigits();
            boolean floating = false;
            if (peek('.')) {
                floating = true;
                index++;
                consumeDigits();
            }
            if (peek('e') || peek('E')) {
                floating = true;
                index++;
                if (peek('+') || peek('-')) {
                    index++;
                }
                consumeDigits();
            }
            String token = text.substring(start, index);
            try {
                if (floating) {
                    return Double.valueOf(token);
                }
                return Long.valueOf(token);
            } catch (NumberFormatException error) {
                throw new LndException("invalid json number: " + token, error);
            }
        }

        private void consumeDigits() throws LndException {
            if (isAtEnd() || !isDigit(text.charAt(index))) {
                throw new LndException("invalid json number at index " + index);
            }
            while (!isAtEnd() && isDigit(text.charAt(index))) {
                index++;
            }
        }

        private void parseLiteral(String literal) throws LndException {
            if (!text.regionMatches(index, literal, 0, literal.length())) {
                throw new LndException("invalid json literal at index " + index);
            }
            index += literal.length();
        }

        private void expect(char expected) throws LndException {
            skipWhitespace();
            if (isAtEnd() || text.charAt(index) != expected) {
                throw new LndException("expected '" + expected + "' at index " + index);
            }
            index++;
        }

        private boolean peek(char expected) {
            return !isAtEnd() && text.charAt(index) == expected;
        }

        private boolean isDigit(char ch) {
            return ch >= '0' && ch <= '9';
        }
    }
}
